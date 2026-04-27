// Open-app idle outreach — A2-compliant Phase 1 substrate-fight architecture.
//
// Mechanism: when the loop fires, Dave is given the floor — system + recent
// history with no new turn appended. He generates whatever his persona +
// context produces. Output is then run through a multi-layer discriminator
// before any user-visible emission. Drops are persisted to outreach_drops
// for forensic review and Phase 3 fine-tune dataset construction.
//
// Layers (in order):
//   L1 — pre-fire heuristic gating (idle threshold, conversation gate, adaptive
//        backoff scaling with consecutive_drops, cap on unanswered reaches)
//   L2 — non-streaming inference (Dave generates the candidate continuation)
//   L3 — heuristic discriminator (length/ack/defer/leak — cheap, deterministic)
//   L4 — LLM-scoring discriminator (separate evaluator persona, A2-compliant)
//   L5 — emission via the unified stream pipeline (paced renderer, single
//        render path per A6) OR persistence to outreach_drops on drop
//
// Drop rate of 70-80% is *expected* on 9B substrate. The phantom-ack prior is
// hostile; the discriminator is the rejection sampler. Phase 3 fine-tune from
// accumulated drops will weaken the prior over time.

use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

use crate::discriminator::{self, DropReason, LLM_SCORE_PASS_THRESHOLD};
use crate::llama_client::LlamaClient;
use crate::memory_assembler;
use crate::persistence::{self, DbHandle, Message};
use crate::prompts;

// L1 tunables
const OUTREACH_TICK_SECONDS: u64 = 30;                // wake every 30s; gate-check is cheap
const DEFAULT_OUTREACH_THRESHOLD_SECONDS: i64 = 300;  // 5 min default; user-tunable via settings
const OUTREACH_THRESHOLD_MIN_SECONDS: i64 = 60;       // 1 min floor (testing)
const OUTREACH_THRESHOLD_MAX_SECONDS: i64 = 15 * 60;  // 15 min ceiling
const OUTREACH_BACKOFF_AFTER_SECONDS: i64 = 3600;     // > 1hr -> idle_worker takes over
const OUTREACH_HISTORY_TURNS: i64 = 30;
const OUTREACH_CONVERSATION_GATE_MESSAGES: i64 = 6;
const OUTREACH_MAX_UNANSWERED_REACHES: i64 = 3;
pub const SETTING_KEY_OUTREACH_THRESHOLD: &str = "outreach_threshold_seconds";

// Adaptive backoff: required gap between consecutive decisions doubles per
// drop, capped. Base equals the user's outreach threshold so the loop quiets
// down proportionally to the testing tempo. Resets on substantive emit or
// new user input.
const ADAPTIVE_CAP_GAP_SECONDS: i64 = 3600;
const ADAPTIVE_DROP_SHIFT_CAP: u32 = 4; // cap left-shift at 4 (16x base)

/// Read the user-tunable outreach threshold from settings, clamped to
/// [MIN, MAX]. Falls back to default on error.
fn current_threshold_seconds(db: &DbHandle) -> i64 {
    let raw = persistence::get_setting_blocking(db, SETTING_KEY_OUTREACH_THRESHOLD).ok().flatten();
    let parsed = raw.and_then(|s| s.parse::<i64>().ok()).unwrap_or(DEFAULT_OUTREACH_THRESHOLD_SECONDS);
    parsed.clamp(OUTREACH_THRESHOLD_MIN_SECONDS, OUTREACH_THRESHOLD_MAX_SECONDS)
}

pub fn spawn(app: AppHandle, db: DbHandle, client: Arc<LlamaClient>) -> oneshot::Sender<()> {
    let (tx, mut rx) = oneshot::channel::<()>();

    tauri::async_runtime::spawn(async move {
        let mut last_decision_at: i64 = 0;
        let mut consecutive_drops: u32 = 0;
        let mut last_seen_user_input: i64 = 0;

        tracing::info!(
            "outreach loop spawned (tick={}s, default_threshold={}s, conv_gate={} msgs, max_unanswered={})",
            OUTREACH_TICK_SECONDS,
            DEFAULT_OUTREACH_THRESHOLD_SECONDS,
            OUTREACH_CONVERSATION_GATE_MESSAGES,
            OUTREACH_MAX_UNANSWERED_REACHES
        );

        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(OUTREACH_TICK_SECONDS)) => {}
                _ = &mut rx => {
                    tracing::info!("outreach loop shutting down");
                    return;
                }
            }

            // Reset consecutive_drops if a new user input has landed since we last checked.
            if let Ok(presence) = persistence::get_presence(&db).await {
                if presence.last_user_input > last_seen_user_input {
                    if last_seen_user_input != 0 {
                        consecutive_drops = 0;
                    }
                    last_seen_user_input = presence.last_user_input;
                }
            }

            match tick(&app, &db, &client, last_decision_at, consecutive_drops).await {
                Ok(TickOutcome::Skip) => {}
                Ok(TickOutcome::Reach) => {
                    last_decision_at = Utc::now().timestamp();
                    consecutive_drops = 0;
                    tracing::info!("outreach: reached out (consecutive_drops reset)");
                }
                Ok(TickOutcome::Drop(reason)) => {
                    last_decision_at = Utc::now().timestamp();
                    consecutive_drops = consecutive_drops.saturating_add(1);
                    tracing::info!(
                        "outreach: dropped ({}, consecutive_drops={})",
                        reason,
                        consecutive_drops
                    );
                }
                Err(e) => tracing::warn!("outreach tick error: {}", e),
            }
        }
    });

    tx
}

#[derive(Debug)]
enum TickOutcome {
    Skip,
    Reach,
    Drop(&'static str),
}

async fn tick(
    app: &AppHandle,
    db: &DbHandle,
    client: &LlamaClient,
    last_decision_at: i64,
    consecutive_drops: u32,
) -> anyhow::Result<TickOutcome> {
    // ---- L1: pre-fire gating ----
    let presence = persistence::get_presence(db).await?;
    let now = Utc::now().timestamp();
    let elapsed = now - presence.last_user_input;

    // Read live threshold from settings each tick. Cheap (single SQLite row).
    let threshold = current_threshold_seconds(db);

    if elapsed < threshold {
        tracing::info!(
            "outreach tick: skip (idle={}s < threshold={}s)",
            elapsed, threshold
        );
        return Ok(TickOutcome::Skip);
    }
    if elapsed > OUTREACH_BACKOFF_AFTER_SECONDS {
        tracing::info!(
            "outreach tick: skip (idle={}s > backoff={}s, idle_worker takes over)",
            elapsed, OUTREACH_BACKOFF_AFTER_SECONDS
        );
        return Ok(TickOutcome::Skip);
    }

    // Adaptive backoff: required gap doubles with each consecutive drop, capped.
    // Base equals the live threshold so testing-tempo (1min) doesn't get
    // stuck behind production-default (5min) backoff.
    let shift = consecutive_drops.min(ADAPTIVE_DROP_SHIFT_CAP);
    let required_gap = (threshold << shift).min(ADAPTIVE_CAP_GAP_SECONDS);
    if last_decision_at > 0 && (now - last_decision_at) < required_gap {
        tracing::info!(
            "outreach tick: skip (adaptive backoff: {}s remaining, consecutive_drops={})",
            required_gap - (now - last_decision_at),
            consecutive_drops
        );
        return Ok(TickOutcome::Skip);
    }

    let conversation_id = match persistence::latest_conversation_id(db).await? {
        Some(id) => id,
        None => {
            tracing::info!("outreach tick: skip (no conversation)");
            return Ok(TickOutcome::Skip);
        }
    };

    let history =
        persistence::load_recent_messages(db, conversation_id, OUTREACH_HISTORY_TURNS).await?;

    if (history.len() as i64) < OUTREACH_CONVERSATION_GATE_MESSAGES {
        tracing::info!(
            "outreach tick: skip (conv_msgs={} < gate={})",
            history.len(), OUTREACH_CONVERSATION_GATE_MESSAGES
        );
        return Ok(TickOutcome::Skip);
    }

    let (prior_count, _latest_at) =
        persistence::outreach_stats_since(db, conversation_id, presence.last_user_input).await?;
    if prior_count >= OUTREACH_MAX_UNANSWERED_REACHES {
        tracing::info!(
            "outreach tick: skip (max unanswered reaches: prior={}, cap={})",
            prior_count, OUTREACH_MAX_UNANSWERED_REACHES
        );
        return Ok(TickOutcome::Skip);
    }

    // All gates passed — log loud so the loop's liveness is visible in dev.log
    tracing::info!(
        "outreach tick: gates passed (idle={}s, conv_msgs={}, prior={}, consecutive_drops={}); generating",
        elapsed, history.len(), prior_count, consecutive_drops
    );

    let history_shape = classify_history_shape(&history);

    // ---- L2: inference (Candidate A — give Dave the floor) ----
    // Use the memory partition so Dave sees anchor + active epochs + recent.
    let all_msgs = persistence::load_all_messages(db, conversation_id).await?;
    let active_epochs = persistence::list_active_epochs(db, conversation_id).await?;
    let canvas = persistence::get_canvas(db, conversation_id).await?;
    let partition = memory_assembler::partition(&all_msgs, &active_epochs, &canvas);
    // No new turn appended. add_generation_prompt: true (default).
    let messages = memory_assembler::build_chat_messages(
        prompts::SYSTEM_PROMPT,
        &partition,
        None,
    );

    // Non-streaming: we discriminate before any user-visible emit. If pass,
    // we synthesize the stream events so the frontend's paced renderer
    // displays it through the unified pipeline (A6).
    let raw = client.complete(messages, 320, 0.85).await?;
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        // The substrate did the rare thing — produced empty. Treat as a clean drop.
        persistence::insert_outreach_drop(
            db,
            conversation_id,
            "",
            "empty",
            true,
            None,
            Some(&history_shape),
            presence.last_user_input,
        )
        .await?;
        return Ok(TickOutcome::Drop("empty"));
    }

    // ---- L3: heuristic discriminator ----
    if let Err(reason) = discriminator::heuristic_pass(trimmed) {
        persistence::insert_outreach_drop(
            db,
            conversation_id,
            trimmed,
            reason.as_str(),
            false,
            None,
            Some(&history_shape),
            presence.last_user_input,
        )
        .await?;
        return Ok(TickOutcome::Drop(reason.as_str()));
    }

    // ---- L4: LLM-scoring discriminator ----
    let score = match discriminator::llm_score(client, trimmed).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("outreach: scorer failed ({}), failing closed", e);
            persistence::insert_outreach_drop(
                db,
                conversation_id,
                trimmed,
                DropReason::ScorerError.as_str(),
                true,
                None,
                Some(&history_shape),
                presence.last_user_input,
            )
            .await?;
            return Ok(TickOutcome::Drop(DropReason::ScorerError.as_str()));
        }
    };

    if score < LLM_SCORE_PASS_THRESHOLD {
        persistence::insert_outreach_drop(
            db,
            conversation_id,
            trimmed,
            DropReason::LlmScore.as_str(),
            true,
            Some(score as i64),
            Some(&history_shape),
            presence.last_user_input,
        )
        .await?;
        return Ok(TickOutcome::Drop(DropReason::LlmScore.as_str()));
    }

    // ---- L5: emit through unified pipeline ----
    // The paced renderer expects a stream-shaped event sequence. We synthesize
    // it from the already-complete text so the frontend renders identically
    // to a chat reply (A6). The text is pushed in small word-sized chunks so
    // the per-character delays in pacedRenderer behave as designed.

    let _ = app.emit("dave:stream_start", ());
    for chunk in word_chunks(trimmed) {
        let _ = app.emit("dave:token", chunk);
        // Tiny inter-chunk yield so the frontend's renderer queue stays paced
        // even though we're emitting fast. Delay is at the char level on the
        // frontend; here we just don't dump it all in one event-loop tick.
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let msg = persistence::insert_message(db, conversation_id, "assistant", trimmed, true).await?;

    let _ = app.emit(
        "dave:stream_end",
        json!({
            "conversationId": conversation_id,
            "fullText": trimmed,
            "messageId": msg.id,
        }),
    );

    tracing::info!(
        "outreach: emitted ({} chars, score={}, shape={})",
        trimmed.len(),
        score,
        history_shape
    );

    Ok(TickOutcome::Reach)
}

/// Split text into word-sized chunks so the paced renderer (which paces at
/// the character level) gets a steady drip rather than one giant event.
fn word_chunks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        // Break on whitespace and punctuation but keep them attached to the
        // preceding chunk so paced renderer sees real punctuation transitions.
        if c == ' ' || c == '\n' {
            // Advance past consecutive whitespace
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\n') {
                j += 1;
            }
            out.push(&text[start..j]);
            start = j;
            i = j;
        } else {
            i += 1;
        }
    }
    if start < bytes.len() {
        out.push(&text[start..]);
    }
    out
}

/// Classify the shape of the most recent message(s) for the outreach_drops
/// `history_shape` column. Used for per-shape filter analysis. No LLM.
fn classify_history_shape(history: &[Message]) -> String {
    let Some(last) = history.last() else {
        return "empty".to_string();
    };
    let lower = last.content.to_lowercase();
    let trimmed = lower.trim();

    if last.role == "user" {
        // Question marker
        if trimmed.contains('?')
            || trimmed.starts_with("how ")
            || trimmed.starts_with("what ")
            || trimmed.starts_with("when ")
            || trimmed.starts_with("why ")
            || trimmed.starts_with("who ")
            || trimmed.starts_with("where ")
        {
            return "user_question".to_string();
        }
        // Conversation enders
        let enders = [
            "bye", "goodbye", "good night", "talk later", "catch you later",
            "going to", "i'm out", "alright i", "later",
        ];
        if enders.iter().any(|e| trimmed.starts_with(e)) {
            return "user_ender".to_string();
        }
        // Short ack-shaped user turn
        let acks = ["yeah", "yes", "ok", "okay", "right", "sure", "got it", "mhm"];
        if trimmed.chars().count() < 24 && acks.iter().any(|a| trimmed.starts_with(a)) {
            return "user_ack".to_string();
        }
        return "user_statement".to_string();
    }

    // role == "assistant"
    if trimmed.contains('?') {
        // Dave asked something the user didn't answer
        return "assistant_question_unanswered".to_string();
    }
    // Trailing-off fragment heuristic: ends without sentence terminator,
    // contains an em dash, ellipsis, or comma right before end.
    if !trimmed.ends_with('.')
        && !trimmed.ends_with('!')
        && !trimmed.ends_with('?')
        && (trimmed.ends_with("...")
            || trimmed.ends_with("—")
            || trimmed.ends_with(",")
            || trimmed.ends_with(";"))
    {
        return "assistant_fragment".to_string();
    }
    "assistant_statement".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, content: &str) -> Message {
        Message {
            id: 0,
            conversation_id: 0,
            role: role.into(),
            content: content.into(),
            created_at: 0,
        }
    }

    #[test]
    fn classifies_user_question() {
        let h = vec![msg("user", "what time does it open?")];
        assert_eq!(classify_history_shape(&h), "user_question");
    }

    #[test]
    fn classifies_user_ender() {
        let h = vec![msg("user", "alright I'm out, going to sleep")];
        assert_eq!(classify_history_shape(&h), "user_ender");
    }

    #[test]
    fn classifies_user_ack() {
        let h = vec![msg("user", "yeah ok")];
        assert_eq!(classify_history_shape(&h), "user_ack");
    }

    #[test]
    fn classifies_user_statement() {
        let h = vec![msg("user", "the brass strip thing you mentioned earlier really stuck with me")];
        assert_eq!(classify_history_shape(&h), "user_statement");
    }

    #[test]
    fn classifies_assistant_question_unanswered() {
        let h = vec![msg("assistant", "have you ever noticed the way it tilts?")];
        assert_eq!(classify_history_shape(&h), "assistant_question_unanswered");
    }

    #[test]
    fn classifies_assistant_fragment() {
        let h = vec![msg("assistant", "the comma, just sitting there—")];
        assert_eq!(classify_history_shape(&h), "assistant_fragment");
    }

    #[test]
    fn word_chunks_splits_reasonably() {
        let chunks = word_chunks("hello world. yes.");
        assert!(chunks.len() >= 2);
        let joined: String = chunks.join("");
        assert_eq!(joined, "hello world. yes.");
    }
}
