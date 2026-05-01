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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

use crate::discriminator::{self, DropReason, LLM_SCORE_PASS_THRESHOLD};
use crate::llama_client::{ChatMessage, LlamaClient};
use crate::memory_assembler;
use crate::persistence::{self, DbHandle, Message};
use crate::prompts;

// L1 tunables
const OUTREACH_TICK_SECONDS: u64 = 30;                // wake every 30s; gate-check is cheap
const DEFAULT_OUTREACH_THRESHOLD_SECONDS: i64 = 180;  // 3 min default (locked in 2026-05-01 from Bo's tuned setting); user-tunable via settings
const OUTREACH_THRESHOLD_MIN_SECONDS: i64 = 60;       // 1 min floor (testing)
const OUTREACH_THRESHOLD_MAX_SECONDS: i64 = 15 * 60;  // 15 min ceiling
const OUTREACH_BACKOFF_AFTER_SECONDS: i64 = 3600;     // > 1hr -> idle_worker takes over
const OUTREACH_HISTORY_TURNS: i64 = 30;
const OUTREACH_CONVERSATION_GATE_MESSAGES: i64 = 6;
const OUTREACH_MAX_UNANSWERED_REACHES: i64 = 3;
pub const SETTING_KEY_OUTREACH_THRESHOLD: &str = "outreach_threshold_seconds";

// Multi-sample tunables. Per-fire we generate N candidates and pick the best.
// The substrate prior on Qwen3.5-9B is hostile to silence — when given a
// near-empty primer (whitespace user turn), the model overwhelmingly produces
// meta-commentary on silence rather than substantive thought. Single-sample
// inference therefore has a high failure rate even when the architecture is
// correct. Multi-sampling gives Dave more swings at landing on substance.
//
// Trade-off: N inference calls per fire instead of 1. Outreach is background
// work; latency is not user-visible. The cost is ~3x GPU-seconds per fire
// at N=3, sustained at low frequency (1-3 fires per hour at 1-min threshold
// with adaptive backoff). On a 9B model at consumer GPU this is negligible
// in absolute terms.
const OUTREACH_SAMPLE_COUNT: u32 = 3;

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

pub fn spawn(
    app: AppHandle,
    db: DbHandle,
    client: Arc<LlamaClient>,
    chat_in_flight: Arc<AtomicBool>,
) -> oneshot::Sender<()> {
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

            // Pass &Arc<LlamaClient> to tick so it can clone the Arc when
            // calling into helpers that need 'static (e.g. fire_deferred_pending).
            match tick(&app, &db, &client, &chat_in_flight, last_decision_at, consecutive_drops).await {
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
    client: &Arc<LlamaClient>,
    chat_in_flight: &Arc<AtomicBool>,
    last_decision_at: i64,
    consecutive_drops: u32,
) -> anyhow::Result<TickOutcome> {
    // Deref the Arc once for synchronous helper calls (generate_and_score_samples,
    // discriminator::llm_score). For spawned-task helpers we clone the Arc.
    let client_ref: &LlamaClient = client.as_ref();
    // ---- Stream-coordination gate (very first check, cheaper than anything else) ----
    // If the chat path is currently streaming a reply to the user, skip this
    // tick entirely. Doing inference now would be wasted: even if the result
    // passed all discriminators, emitting it would race with the chat stream
    // on the frontend's pacedRenderer. The chat path always wins; outreach
    // yields. Without this, concurrent stream_start events reset
    // pendingAssistant mid-render and produce visible garbage.
    if chat_in_flight.load(Ordering::SeqCst) {
        tracing::info!("outreach tick: skip (chat path in flight)");
        return Ok(TickOutcome::Skip);
    }

    // ---- Deferred chat-response fire — FALLBACK PATH ----
    //
    // The primary path is the precise spawned timer in send_to_dave's Delay
    // branch (zero drift, fires exactly at +seconds). This outreach-tick
    // sweep is the fallback for the edge case where the spawned timer was
    // lost — typically app restart with un-fired pendings still in the DB.
    //
    // fire_deferred_pending does an atomic claim, so if both paths happen
    // to race, only one wins. Safe to call from both.
    if let Ok(due) = persistence::due_pending_responses(db).await {
        for pending in due {
            // Both the spawned timer (in send_to_dave's Delay branch) and
            // this fallback call fire_deferred_pending. The helper does an
            // atomic claim via persistence::claim_pending_fire — only one
            // wins and runs; the other no-ops. Safe to call from both.
            crate::commands::fire_deferred_pending(
                app.clone(),
                db.clone(),
                client.clone(),
                chat_in_flight.clone(),
                pending.id,
            )
            .await;
        }
    }

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

    // History-shape tagging (for outreach_drops forensics only — no longer gates).
    // The chat-template-shape issue (Qwen3.5-9B produces empty when history
    // ends on assistant) is now solved at the inference layer below by always
    // appending a whitespace user turn, which normalizes the prompt shape into
    // "user said nothing, generate assistant" regardless of who spoke last.
    // This lets outreach fire after Dave's own turns — which is the actual
    // semantic we want: outreach is for breaking silences in general, not
    // only silences the user opened. Dave thinking of something three minutes
    // after his own reply is a real conversational move, not a glitch.
    let history_shape = classify_history_shape(&history);

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
        "outreach tick: gates passed (idle={}s, conv_msgs={}, prior={}, consecutive_drops={}, shape={}); generating",
        elapsed, history.len(), prior_count, consecutive_drops, history_shape
    );

    // ---- L2: multi-sample inference (Candidate B with N=OUTREACH_SAMPLE_COUNT) ----
    //
    // Use the memory partition so Dave sees anchor + active epochs + recent,
    // then append a whitespace user turn. This normalizes the chat-template
    // prompt shape into "user said nothing, generate assistant" — sane
    // regardless of who spoke last in the conversation. Without it, when
    // history ends on an assistant turn, Qwen3.5-9B produces empty 100% of
    // the time (verified empirically across days of accumulated drops).
    //
    // Multi-sample rationale: the substrate prior on a 9B chat model is
    // hostile to "user said nothing → produce substance." Most samples land
    // on meta-commentary about silence. Generating N candidates per fire and
    // picking the highest-scoring one substantially improves the chance of
    // landing on a substantive thought (etymology, marginalia, abandoned
    // infrastructure — Dave's persona-prompt interest list) versus
    // substrate-honesty meta-talk that the discriminator correctly rejects.
    let all_msgs = persistence::load_all_messages(db, conversation_id).await?;
    let active_epochs = persistence::list_active_epochs(db, conversation_id).await?;
    let canvas = persistence::get_canvas(db, conversation_id).await?;
    let partition = memory_assembler::partition(&all_msgs, &active_epochs, &canvas);
    let messages = memory_assembler::build_chat_messages(
        prompts::SYSTEM_PROMPT,
        &partition,
        Some(" "),
    );

    tracing::info!(
        "outreach: multi-sample fire begin (N={}, shape={})",
        OUTREACH_SAMPLE_COUNT, history_shape
    );

    // Each candidate runs through the full discriminator chain (heuristic +
    // LLM scorer). We track the result so we can pick the best at the end.
    let candidates = generate_and_score_samples(
        client_ref,
        &messages,
        OUTREACH_SAMPLE_COUNT,
    )
    .await;

    // Log the per-sample scores for forensic analysis (variance across samples
    // tells us whether the substrate-prior is uniform-bad or sometimes-good
    // for this fire's prompt context).
    log_multi_sample_summary(&candidates);

    // Pick the best candidate that passed heuristic AND received a score.
    // Among those, take the one with the highest score. Ties broken by
    // first-seen.
    let best = pick_best_candidate(&candidates);

    match best {
        Some(idx) if candidates[idx].score.unwrap_or(0) >= LLM_SCORE_PASS_THRESHOLD => {
            // PASS heuristic + LLM-score, but first run two more pre-emit gates.
            //
            // Gate A: dedup against most recent outreach. The substrate prior
            // can land on near-identical opening sentences across consecutive
            // fires when the conversation context hasn't changed. Empirically
            // observed: two reaches differing only by an inserted "still" in
            // the first sentence. Immersion-breaking — looks like Dave doesn't
            // see his own prior message. Compare to the most recent
            // initiated_by_dave message and drop if too similar.
            if let Ok(Some(prior_reach)) =
                persistence::last_dave_initiated_message(db, conversation_id).await
            {
                if discriminator::is_too_similar_to_last_reach(
                    &candidates[idx].content,
                    &prior_reach,
                ) {
                    tracing::info!(
                        "outreach: drop (duplicate_reach: candidate too similar to prior reach, sample {}/{} score={})",
                        idx + 1, OUTREACH_SAMPLE_COUNT, candidates[idx].score.unwrap_or(0)
                    );
                    persist_drop(
                        db, conversation_id, &candidates[idx],
                        DropReason::DuplicateReach, &history_shape, presence.last_user_input,
                    )
                    .await?;
                    persist_losing_candidates(
                        db, conversation_id, &candidates, idx,
                        &history_shape, presence.last_user_input,
                    )
                    .await;
                    return Ok(TickOutcome::Drop("duplicate_reach"));
                }
            }

            // Gate B: stream-coordination — chat path may have started during
            // multi-sample inference (which can take ~10s on a 9B model with
            // N=3 samples). If so, the user is mid-conversation right now and
            // emitting outreach would race with the chat stream. Drop our
            // result silently to avoid clobbering the chat pane mid-render.
            // The inference work is wasted but the user-facing experience
            // stays clean.
            if chat_in_flight.load(Ordering::SeqCst) {
                tracing::info!(
                    "outreach: emit-time abort (chat path started during multi-sample inference, sample {}/{} score={} discarded)",
                    idx + 1, OUTREACH_SAMPLE_COUNT, candidates[idx].score.unwrap_or(0)
                );
                // Log all candidates as concurrent_chat drops for forensics
                // — useful to know how often we lose a substantive reach to
                // chat-path overlap.
                for (i, c) in candidates.iter().enumerate() {
                    let reason_str = if i == idx {
                        "concurrent_chat_winner"
                    } else {
                        "concurrent_chat_loser"
                    };
                    let _ = persistence::insert_outreach_drop(
                        db,
                        conversation_id,
                        &c.content,
                        reason_str,
                        c.heuristic_result.is_ok(),
                        c.score.map(|s| s as i64),
                        Some(&history_shape),
                        presence.last_user_input,
                    )
                    .await;
                }
                return Ok(TickOutcome::Drop("concurrent_chat"));
            }

            // Emit the best candidate through the unified pipeline.
            let winner = &candidates[idx];
            let content = winner.content.clone();
            let score = winner.score.unwrap_or(0);

            tracing::info!(
                "outreach: emitting sample {}/{} (score={}, {} chars, shape={})",
                idx + 1, OUTREACH_SAMPLE_COUNT, score, content.len(), history_shape
            );

            // The paced renderer expects a stream-shaped event sequence. We
            // synthesize it from the complete text so the frontend renders
            // identically to a chat reply (A6). Word-sized chunks let the
            // per-character delays in pacedRenderer behave as designed.
            let _ = app.emit("dave:stream_start", ());
            for chunk in word_chunks(&content) {
                // Mid-emit defense: if user sends a message DURING our
                // emission (rare given emission completes in ~1s), abort
                // cleanly so the chat path can take the floor. Frontend
                // handles dave:stream_aborted by clearing pendingAssistant.
                if chat_in_flight.load(Ordering::SeqCst) {
                    tracing::info!(
                        "outreach: mid-emit abort (chat path started while emitting; sample {}/{})",
                        idx + 1, OUTREACH_SAMPLE_COUNT
                    );
                    let _ = app.emit("dave:stream_aborted", ());
                    return Ok(TickOutcome::Drop("interrupted_by_chat"));
                }
                let _ = app.emit("dave:token", chunk);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            let msg = persistence::insert_message(
                db, conversation_id, "assistant", &content, true,
            )
            .await?;

            let _ = app.emit(
                "dave:stream_end",
                json!({
                    "conversationId": conversation_id,
                    "fullText": content.clone(),
                    "messageId": msg.id,
                }),
            );

            // Log the losing candidates to drops for forensic analysis (their
            // content plus the fact that they lost the within-fire competition
            // is useful Phase 3 fine-tune signal).
            persist_losing_candidates(
                db, conversation_id, &candidates, idx,
                &history_shape, presence.last_user_input,
            )
            .await;

            Ok(TickOutcome::Reach)
        }
        Some(idx) => {
            // BEST CANDIDATE EXISTS BUT BELOW THRESHOLD — log it as the
            // representative drop with the actual reason (llm_score). Log the
            // others as losing-sample drops for completeness.
            let representative = &candidates[idx];
            let reason = match representative.heuristic_result {
                Err(r) => r,
                Ok(()) => DropReason::LlmScore,
            };
            tracing::info!(
                "outreach: drop (best of {}: sample {} reason={} score={:?}, shape={})",
                OUTREACH_SAMPLE_COUNT, idx + 1, reason.as_str(),
                representative.score, history_shape
            );

            persist_drop(
                db, conversation_id, representative,
                reason, &history_shape, presence.last_user_input,
            )
            .await?;

            // Log the losing candidates too so the full multi-sample picture
            // is in the drops table.
            persist_losing_candidates(
                db, conversation_id, &candidates, idx,
                &history_shape, presence.last_user_input,
            )
            .await;

            Ok(TickOutcome::Drop(reason.as_str()))
        }
        None => {
            // NO CANDIDATE PASSED HEURISTIC — log all as drops. Common case:
            // all 3 samples were empty (chat template didn't fire correctly)
            // or all 3 failed length/ack/defer.
            tracing::info!(
                "outreach: drop (all {} samples failed heuristic, shape={})",
                OUTREACH_SAMPLE_COUNT, history_shape
            );

            // Use the first candidate's reason as the representative outcome.
            let representative_reason = candidates
                .first()
                .and_then(|c| c.heuristic_result.err())
                .unwrap_or(DropReason::Empty);

            for c in &candidates {
                let reason = c.heuristic_result.err().unwrap_or(DropReason::Empty);
                persist_drop(
                    db, conversation_id, c,
                    reason, &history_shape, presence.last_user_input,
                )
                .await?;
            }

            Ok(TickOutcome::Drop(representative_reason.as_str()))
        }
    }
}

/// One candidate produced by a single inference sample. Carries the content,
/// the heuristic outcome, and (if heuristic passed) the LLM score.
struct Candidate {
    /// Trimmed model output. Empty string if inference returned empty.
    content: String,
    /// Result of the heuristic discriminator. Ok if it passed, Err with the
    /// reason if it failed (length / ack-only / defer / leak).
    heuristic_result: Result<(), DropReason>,
    /// LLM-scoring discriminator score (0-9), or None if heuristic failed
    /// (no point scoring something that already failed gating) or if the
    /// scorer call itself errored.
    score: Option<u8>,
}

/// Generate N samples in sequence, running each through the discriminator
/// chain. Sequential rather than parallel because llama-server has a single
/// slot for our use case; parallel calls would just queue.
async fn generate_and_score_samples(
    client: &LlamaClient,
    messages: &[ChatMessage],
    n: u32,
) -> Vec<Candidate> {
    let mut out = Vec::with_capacity(n as usize);

    for sample_idx in 1..=n {
        let raw = match client.complete(messages.to_vec(), 320, 0.85).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "outreach: inference error on sample {}/{}: {}",
                    sample_idx, n, e
                );
                out.push(Candidate {
                    content: String::new(),
                    heuristic_result: Err(DropReason::ScorerError),
                    score: None,
                });
                continue;
            }
        };
        let trimmed = raw.trim().to_string();

        if trimmed.is_empty() {
            tracing::info!("outreach: sample {}/{} produced empty", sample_idx, n);
            out.push(Candidate {
                content: trimmed,
                heuristic_result: Err(DropReason::Empty),
                score: None,
            });
            continue;
        }

        // Heuristic discriminator (cheap, deterministic).
        let heuristic_result = discriminator::heuristic_pass(&trimmed);

        // LLM scorer only runs if heuristic passed (no point scoring something
        // that was going to be rejected anyway).
        let score = match heuristic_result {
            Ok(()) => match discriminator::llm_score(client, &trimmed).await {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!(
                        "outreach: scorer failed on sample {}/{}: {}",
                        sample_idx, n, e
                    );
                    None
                }
            },
            Err(_) => None,
        };

        tracing::info!(
            "outreach: sample {}/{} content_len={} heuristic={} score={:?}",
            sample_idx, n, trimmed.chars().count(),
            match &heuristic_result {
                Ok(()) => "pass",
                Err(r) => r.as_str(),
            },
            score
        );

        out.push(Candidate {
            content: trimmed,
            heuristic_result,
            score,
        });
    }

    out
}

/// Find the index of the best candidate. Best = passed heuristic AND has
/// a score; among qualifying, highest score wins; ties broken by first-seen.
/// Returns None if no candidate qualifies.
fn pick_best_candidate(candidates: &[Candidate]) -> Option<usize> {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.heuristic_result.is_ok() && c.score.is_some())
        .max_by_key(|(_, c)| c.score.unwrap_or(0))
        .map(|(i, _)| i)
}

/// Emit a one-line summary of the multi-sample fire's scoring distribution.
/// Useful when grepping dev.log to understand why fires are succeeding or
/// failing.
fn log_multi_sample_summary(candidates: &[Candidate]) {
    let scores: Vec<String> = candidates
        .iter()
        .map(|c| match (&c.heuristic_result, c.score) {
            (Ok(()), Some(s)) => format!("{}", s),
            (Ok(()), None) => "scorer_err".to_string(),
            (Err(r), _) => format!("h:{}", r.as_str()),
        })
        .collect();
    tracing::info!(
        "outreach: multi-sample fire result: scores=[{}]",
        scores.join(", ")
    );
}

/// Insert a single drop row.
async fn persist_drop(
    db: &DbHandle,
    conversation_id: i64,
    candidate: &Candidate,
    reason: DropReason,
    history_shape: &str,
    last_user_input: i64,
) -> anyhow::Result<()> {
    let heuristic_pass = candidate.heuristic_result.is_ok();
    persistence::insert_outreach_drop(
        db,
        conversation_id,
        &candidate.content,
        reason.as_str(),
        heuristic_pass,
        candidate.score.map(|s| s as i64),
        Some(history_shape),
        last_user_input,
    )
    .await?;
    Ok(())
}

/// When a candidate wins the within-fire competition, log the others (the
/// "lost" candidates) as `lost_to_better_sample` drops. This preserves the
/// full forensic picture: not just "what won" but "what didn't, and why."
async fn persist_losing_candidates(
    db: &DbHandle,
    conversation_id: i64,
    candidates: &[Candidate],
    winner_idx: usize,
    history_shape: &str,
    last_user_input: i64,
) {
    for (i, c) in candidates.iter().enumerate() {
        if i == winner_idx {
            continue;
        }
        // Reason: if this candidate failed heuristic, use that reason;
        // otherwise mark it as "lost_to_better_sample" (it was a viable
        // candidate but didn't have the highest score).
        let reason_str: &str = match c.heuristic_result {
            Err(r) => r.as_str(),
            Ok(()) => "lost_to_better_sample",
        };
        let heuristic_pass = c.heuristic_result.is_ok();
        if let Err(e) = persistence::insert_outreach_drop(
            db,
            conversation_id,
            &c.content,
            reason_str,
            heuristic_pass,
            c.score.map(|s| s as i64),
            Some(history_shape),
            last_user_input,
        )
        .await
        {
            tracing::warn!("outreach: failed to log losing candidate {}: {}", i + 1, e);
        }
    }
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
