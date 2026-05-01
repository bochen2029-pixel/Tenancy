use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use chrono::Local;
use chrono::Utc;

use crate::chat_pacing;
use crate::chat_triage::{self, ChatDecision};
use crate::leak;
use crate::llama_client::ChatMessage;
use crate::memory_assembler;
use crate::persistence::{self, ConsolidationEpoch, JournalEntry, MemoryEdit, Message, OutreachDrop};
use crate::prompts;
use crate::time_awareness;
use crate::AppState;

/// After this many consecutive user messages with no assistant reply, the
/// triage layer is bypassed and the next send forces RESPOND. Mirrors the
/// "you keep talking, I'll answer" social pressure — the harness should not
/// be able to refuse indefinitely.
const FORCED_RESPOND_THRESHOLD: usize = 3;

/// Read the pace factor from settings, clamped to safe range. Default 1.0
/// when missing or unparseable.
pub(crate) async fn read_pace_setting(db: &crate::persistence::DbHandle) -> f32 {
    let raw = persistence::get_setting(db, chat_pacing::SETTING_KEY_PACE)
        .await
        .ok()
        .flatten()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(chat_pacing::PACE_DEFAULT);
    chat_pacing::clamp_pace(raw)
}

/// Fire a deferred-pending chat response (the second half of a Delay
/// decision). Atomically claims the pending row; if claim succeeds, runs
/// inference + emission with current cadence-aware pacing. If the row was
/// already cancelled (user sent a new message) or already fired (race with
/// another claimer), skips silently.
///
/// Called from two paths, both compete for the claim:
///   1. Precise per-pending tokio timer (spawned in send_to_dave's Delay
///      branch) — wins the race in the normal case.
///   2. Outreach loop's tick fallback — wins only if the spawned timer was
///      lost (app restart, panic). Guarantees no pending lingers forever.
pub(crate) async fn fire_deferred_pending(
    app: tauri::AppHandle,
    db: crate::persistence::DbHandle,
    client: Arc<crate::llama_client::LlamaClient>,
    chat_in_flight: Arc<AtomicBool>,
    pending_id: i64,
) {
    // Atomic claim. Only one caller wins; the other sees fired=1 and bails.
    let claimed = match persistence::claim_pending_fire(&db, pending_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("fire_deferred_pending: claim failed: {}", e);
            return;
        }
    };
    if !claimed {
        // Already fired (other claimer won the race) or cancelled (user
        // sent new message). No-op.
        tracing::debug!("fire_deferred_pending: pending {} already claimed/cancelled", pending_id);
        return;
    }

    // Look up the row to get user_message_id + conversation_id.
    let pending = match persistence::load_pending_by_id(&db, pending_id).await {
        Ok(Some(p)) => p,
        _ => {
            tracing::warn!("fire_deferred_pending: pending row {} missing", pending_id);
            return;
        }
    };

    // Look up the user message that triggered this pending.
    let user_msg = match persistence::load_message_by_id(&db, pending.user_message_id).await {
        Ok(Some(m)) if m.role == "user" => m,
        _ => {
            tracing::warn!(
                "fire_deferred_pending: pending {} references missing/non-user message {}",
                pending.id, pending.user_message_id
            );
            return;
        }
    };

    let now = chrono::Utc::now().timestamp();
    tracing::info!(
        "fire_deferred_pending: firing pending_id={} conv={} scheduled_lag={}s",
        pending.id,
        pending.conversation_id,
        now - pending.fire_at,
    );

    // Load recent history for cadence-aware pacing.
    let recent_msgs = persistence::load_recent_messages(&db, pending.conversation_id, 30)
        .await
        .unwrap_or_default();

    let pace = read_pace_setting(&db).await;

    // Emit stream_start (typing indicator) and call the shared chat helper.
    let _ = app.emit("dave:stream_start", ());
    if let Err(e) = run_chat_inference_and_emit(
        &app,
        &db,
        client.as_ref(),
        &chat_in_flight,
        pending.conversation_id,
        &user_msg.content,
        &recent_msgs,
        pace,
    )
    .await
    {
        tracing::warn!("fire_deferred_pending: inference error: {}", e);
    }
}

/// RAII guard that marks `chat_in_flight = true` for the duration of a chat
/// stream and clears it on drop. Use via `let _g = ChatInFlightGuard::new(...);`
/// at the top of any code path that emits to the frontend. Outreach checks
/// this flag to avoid stomping on the chat path's stream.
struct ChatInFlightGuard {
    flag: Arc<AtomicBool>,
}

impl ChatInFlightGuard {
    fn new(flag: Arc<AtomicBool>) -> Self {
        flag.store(true, Ordering::SeqCst);
        Self { flag }
    }
}

impl Drop for ChatInFlightGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

const VIEW_LOAD_LIMIT: i64 = 200;

#[tauri::command]
pub async fn send_to_dave(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: i64,
    user_text: String,
) -> Result<(), String> {
    persistence::touch_user_input(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    // A new user message supersedes any prior pending deferred response for
    // this conversation. The Delay path schedules a fire at T+N seconds; if
    // the user types again before the fire, that prior pending is stale —
    // the new message is the live signal. Cancelled rows stay in the table
    // for forensics.
    let cancelled = persistence::cancel_pending_for_conversation(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    if cancelled > 0 {
        tracing::info!(
            "send_to_dave: cancelled {} pending response(s) for conversation {}",
            cancelled, conversation_id
        );
    }

    // Persist the user message FIRST so triage and the deferred fire path
    // both have a stable user_message_id to reference. The chat_decisions
    // and pending_chat_responses rows both FK on this id.
    let user_msg =
        persistence::insert_message(&state.db, conversation_id, "user", &user_text, false)
            .await
            .map_err(|e| e.to_string())?;
    let _ = app.emit("dave:user_persisted", &user_msg);

    // ============================================================
    // Delivery + read indicators (Telegram-style two checkmarks)
    // ============================================================
    //
    // First checkmark — DELIVERED. Real, not fake: probes llama-server's
    // /health endpoint. If the harness can't reach the model, no first
    // checkmark fires. The user sees "still sending" until connectivity
    // returns. If llama-server is up (the typical case), the first
    // checkmark fires within ~50ms of send.
    let connected = state.client.health_check().await;
    if connected {
        let _ = app.emit("dave:message_delivered", json!({ "messageId": user_msg.id }));
    } else {
        tracing::warn!(
            "send_to_dave: llama-server health check failed — message not marked delivered"
        );
        // No delivered emit. The frontend will still render the message
        // (optimistic) but the checkmark stays empty until... well, until
        // the user tries again or restarts. We don't retry here; this is
        // the rare disconnect case.
    }

    // Load recent history BEFORE the read delay — we need it to compute
    // a cadence-aware read delay. Rapid chitchat (last 6 messages each
    // within 30s of the previous) → near-instant read. Slow exchange
    // (gaps >120s) → full read time.
    let recent_for_triage =
        persistence::load_recent_messages(&state.db, conversation_id, 30)
            .await
            .map_err(|e| e.to_string())?;

    // Pace factor — global timing multiplier, settable via SettingsPanel.
    // 1.0 = baseline. 0.2x = snappy. 2.0x = deliberate. Read once per
    // send for log clarity; reused by run_chat_inference_and_emit for the
    // post-decision pacing.
    let pace = read_pace_setting(&state.db).await;

    // Second checkmark — READ. Real, not fake: this is the genuine "Dave
    // is ingesting your message into his pipeline" period. The harness
    // sleeps for a cadence + length + pace proportional duration, then
    // triage actually runs against the message. The read mark fires AT
    // triage decision — the moment Dave's processing has actually engaged
    // with the message.
    //
    // If Bo later adds a "Dave timeout" state (refusing to engage at all
    // for some duration), this is where we'd skip the read emit entirely
    // — the message would stay at "delivered" with no read mark for the
    // duration of the timeout.
    let read_delay_ms = chat_pacing::compute_read_delay_ms(&user_text, &recent_for_triage, pace);
    tracing::info!(
        "send_to_dave: read delay {}ms (cadence={:.2}, pace={:.2})",
        read_delay_ms,
        chat_pacing::compute_cadence_score(&recent_for_triage),
        pace,
    );
    tokio::time::sleep(std::time::Duration::from_millis(read_delay_ms)).await;

    // Triage — heuristic-only, no LLM. Most messages produce zero weights and
    // fast-lane to RESPOND. Hostile / repetitive / demanding messages produce
    // small Delay/Refuse weights, capped at 30%/10%.
    //
    // Force-respond override: after FORCED_RESPOND_THRESHOLD consecutive user
    // messages with no Dave reply, the harness bypasses triage. Models the
    // social pressure of "you keep talking — I'll answer." Without this, a
    // run of bad-luck Delays/Refuses could leave the user shouting into the
    // void indefinitely.
    let unanswered = chat_triage::consecutive_unanswered_user_messages(&recent_for_triage);
    let triage = chat_triage::triage(&user_text, &recent_for_triage);

    // Settings-driven testing override. When set to "delay" / "refuse" /
    // "respond", forces that decision regardless of triage weights. Useful
    // for verifying the deferred-fire path during manual testing without
    // having to roll the dice on probabilistic triage. Empty/unset = normal
    // weighted-sampling path.
    let force_setting = persistence::get_setting(&state.db, "chat_triage_force")
        .await
        .ok()
        .flatten()
        .unwrap_or_default();

    let decision = if unanswered >= FORCED_RESPOND_THRESHOLD {
        tracing::info!(
            "send_to_dave: forcing respond ({} consecutive unanswered user msgs)",
            unanswered
        );
        ChatDecision::ForcedRespond
    } else {
        match force_setting.as_str() {
            "delay" => {
                tracing::info!("send_to_dave: chat_triage_force=delay (test override)");
                ChatDecision::Delay {
                    seconds: 5, // short test window. The deferred-fire's own
                                // chat_pacing run will add its own compose +
                                // streaming time on top of this 5s gate.
                }
            }
            "refuse" => {
                tracing::info!("send_to_dave: chat_triage_force=refuse (test override)");
                ChatDecision::Refuse
            }
            "respond" => {
                tracing::info!("send_to_dave: chat_triage_force=respond (test override)");
                ChatDecision::Respond
            }
            _ => chat_triage::decide(&triage),
        }
    };

    // Always log the triage outcome so you can see chat_triage IS running
    // even when the dice land on Respond. Without this it's invisible by
    // design (Respond is the silent fast path).
    tracing::info!(
        "send_to_dave: triage decision={} reasons={} weights=(d={:.2}, r={:.2}) unanswered={}",
        decision.as_str(),
        triage.reasons_str(),
        triage.delay_weight,
        triage.refuse_weight,
        unanswered
    );

    // SECOND CHECKMARK fires here — Dave's pipeline has actually completed
    // reading + triage. From this moment onward Dave is in one of three
    // states (Respond / Delay / Refuse). The user can see Dave has read
    // their message regardless of which branch is taken.
    let _ = app.emit("dave:message_read", json!({ "messageId": user_msg.id }));

    // Record the decision regardless of branch, so the chat_decisions table
    // accumulates the full distribution (respond/delay/refuse/forced_respond)
    // for forensic review and Phase-3 fine-tune dataset construction.
    let delay_seconds_for_log: Option<i64> = match &decision {
        ChatDecision::Delay { seconds } => Some(*seconds as i64),
        _ => None,
    };
    persistence::insert_chat_decision(
        &state.db,
        conversation_id,
        user_msg.id,
        decision.as_str(),
        &triage.reasons_str(),
        triage.delay_weight,
        triage.refuse_weight,
        delay_seconds_for_log,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Branch.
    match decision {
        ChatDecision::Respond | ChatDecision::ForcedRespond => {
            // Emit stream_start NOW — TypingIndicator dots appear immediately
            // after the second checkmark. This is Bo's "if dave decides to
            // type at all, the typing indicator will ALWAYS precede any
            // words showing up at all" directive (2026-04-30).
            //
            // The cadence-aware pacing inside run_chat_inference_and_emit
            // computes T_compose (indicator visible duration) from the
            // response length and conversation cadence after inference
            // completes. Inference time itself counts toward T_compose;
            // any extra hold is sleep'd before tokens emit.
            let _ = app.emit("dave:stream_start", ());

            run_chat_inference_and_emit(
                &app,
                &state.db,
                state.client.as_ref(),
                &state.chat_in_flight,
                conversation_id,
                &user_text,
                &recent_for_triage,
                pace,
            )
            .await
        }
        ChatDecision::Delay { seconds } => {
            // No stream_start emit yet — typing indicator only appears when
            // Dave is actually about to type. The spawned precise timer
            // below emits stream_start when it fires.
            //
            // We emit dave:stream_aborted now so the frontend's isStreaming
            // state (set true on send) gets reset and the composer unlocks.
            // The user can absolutely send more messages — a Delay is about
            // Dave's response timing, not a gate on the user.
            let fire_at = Utc::now().timestamp() + seconds as i64;
            let pending_id = persistence::schedule_pending_response(
                &state.db,
                conversation_id,
                user_msg.id,
                fire_at,
            )
            .await
            .map_err(|e| e.to_string())?;
            tracing::info!(
                "send_to_dave: delay decision (seconds={}, pending_id={}, reasons={}, weights d={:.2}/r={:.2})",
                seconds, pending_id, triage.reasons_str(), triage.delay_weight, triage.refuse_weight
            );
            let _ = app.emit("dave:stream_aborted", ());

            // Spawn a precise per-pending timer. Sleeps EXACTLY `seconds`
            // and then fires. Replaces the old "wait for outreach tick to
            // notice" mechanism, which had up-to-30s drift (Bo flagged
            // 2026-05-01: scheduled_lag=20s on a 5s delay).
            //
            // The fire_deferred_pending helper does an atomic claim, so if
            // outreach tick happens to also pick this up at the same time
            // (extremely rare) only one of them actually runs.
            let app_clone = app.clone();
            let db_clone = state.db.clone();
            let client_clone = state.client.clone();
            let chat_in_flight_clone = state.chat_in_flight.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                fire_deferred_pending(
                    app_clone,
                    db_clone,
                    client_clone,
                    chat_in_flight_clone,
                    pending_id,
                )
                .await;
            });

            Ok(())
        }
        ChatDecision::Refuse => {
            // No stream of any kind. The user's message lands in the pane;
            // Dave does not respond. As with Delay, emit stream_aborted so
            // the frontend's isStreaming flag clears and the composer
            // unlocks — Dave can refuse to reply, but he cannot prevent the
            // user from sending more messages.
            tracing::info!(
                "send_to_dave: refuse decision (reasons={}, weights d={:.2}/r={:.2})",
                triage.reasons_str(), triage.delay_weight, triage.refuse_weight
            );
            let _ = app.emit("dave:stream_aborted", ());
            Ok(())
        }
    }
}

/// The "actually run inference and emit" half of the chat path. Shared by
/// the immediate-respond branch of send_to_dave and by the deferred-fire
/// path in outreach (when a Delay'd pending response comes due).
///
/// Takes raw fields (not a State<AppState>) so outreach can call it from
/// its tokio task context where State isn't available.
///
/// Preconditions: the user message has already been persisted (its content is
/// the last entry in the conversation's recent slice). The caller has emitted
/// dave:stream_start (TypingIndicator visible). This function:
///   1. Runs inference WITHOUT per-token emit (collect-only).
///   2. Computes cadence-aware ChatPacing from response length + recent_msgs.
///   3. Sleeps any extra hold needed to honor T_compose target.
///   4. Emits each char as a dave:token event with calculated per-char delay
///      (variable, with punctuation pauses).
///   5. Persists + emits dave:stream_end.
///
/// All visual pacing is owned by the backend. The frontend pacedRenderer
/// becomes a thin pass-through.
pub(crate) async fn run_chat_inference_and_emit(
    app: &AppHandle,
    db: &crate::persistence::DbHandle,
    client: &crate::llama_client::LlamaClient,
    chat_in_flight: &Arc<AtomicBool>,
    conversation_id: i64,
    user_text: &str,
    recent_msgs: &[Message],
    pace: f32,
) -> Result<(), String> {
    // Hold the chat-in-flight flag for the entire send. Outreach loop checks
    // this both before its inference and before its emission, so it won't
    // race with us. Released automatically on drop (any return path).
    let _chat_guard = ChatInFlightGuard::new(chat_in_flight.clone());

    // Assemble context with memory partition: anchor + active epochs +
    // un-consolidated middle + recent.
    let all_msgs = persistence::load_all_messages(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let active_epochs = persistence::list_active_epochs(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let canvas = persistence::get_canvas(db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let partition = memory_assembler::partition(&all_msgs, &active_epochs, &canvas);

    // Conditional time-awareness: if the user's message reaches for time,
    // prepend a single ambient sentence to the system prompt for THIS
    // request only. Persistent context stays clean.
    let system_content = if time_awareness::user_message_invokes_time(user_text) {
        let now = Local::now();
        time_awareness::system_prompt_with_time(prompts::SYSTEM_PROMPT, &now)
    } else {
        prompts::SYSTEM_PROMPT.to_string()
    };

    // user_text is already in partition.recent (persisted earlier in
    // send_to_dave or by the original send for the deferred fire). Pass None
    // to build_chat_messages so it doesn't append a duplicate user turn.
    let messages = memory_assembler::build_chat_messages(
        &system_content,
        &partition,
        None,
    );

    // Run inference WITHOUT per-token emit. Collect into the full result.
    // The backend takes full ownership of pacing — frontend just renders.
    let inference_start = std::time::Instant::now();
    let full = client
        .chat_stream(messages, |_tok| {
            // No-op: the per-char emission happens after pacing is computed.
        })
        .await
        .map_err(|e| e.to_string())?;
    let inference_elapsed = inference_start.elapsed();

    // Defense in depth: if Dave's response is a harness leak, drop it.
    if leak::is_harness_leak(&full) {
        tracing::warn!("run_chat_inference_and_emit: dropping harness leak from response");
        let _ = app.emit("dave:stream_aborted", ());
        return Ok(());
    }

    // Empty or whitespace-only response — abort cleanly.
    let trimmed = full.trim();
    if trimmed.is_empty() {
        tracing::warn!("run_chat_inference_and_emit: empty response, aborting");
        let _ = app.emit("dave:stream_aborted", ());
        return Ok(());
    }

    // Compute cadence-aware pacing.
    let response_chars = full.chars().count();
    let pacing = chat_pacing::compute_chat_pacing(response_chars, recent_msgs, pace);
    tracing::info!(
        "run_chat_inference_and_emit: pacing chars={} cadence={:.2} pace={:.2} typing_speed={:.1}cps t_total={}ms t_compose={}ms (ratio={:.2}) t_stream={}ms char_base={}ms",
        pacing.response_chars,
        pacing.cadence_score,
        pace,
        pacing.typing_speed_chars_per_sec,
        pacing.t_total_ms,
        pacing.compose_hold_ms,
        pacing.compose_ratio,
        pacing.t_streaming_ms,
        pacing.char_base_ms,
    );

    // Hold the TypingIndicator for the compose phase. Inference time
    // already elapsed counts toward T_compose (Dave was thinking while
    // the model was inferring, conceptually). Sleep any remaining time.
    let elapsed_ms = inference_elapsed.as_millis() as u64;
    let extra_hold_ms = pacing.compose_hold_ms.saturating_sub(elapsed_ms);
    if extra_hold_ms > 0 {
        tracing::info!(
            "run_chat_inference_and_emit: extra compose hold {}ms (inference {}ms < target {}ms)",
            extra_hold_ms, elapsed_ms, pacing.compose_hold_ms
        );
        tokio::time::sleep(std::time::Duration::from_millis(extra_hold_ms)).await;
    } else {
        tracing::info!(
            "run_chat_inference_and_emit: no extra hold (inference {}ms >= target {}ms)",
            elapsed_ms, pacing.compose_hold_ms
        );
    }

    // Emit chars one by one, each at an ABSOLUTE target time. Using
    // `tokio::time::sleep_until(target_instant)` instead of relative
    // `sleep(duration)` compensates for tokio runtime + Windows timer
    // granularity (~15ms minimum slice) + Tauri IPC overhead per emit.
    //
    // Without this, each "sleep N ms" returns ~N + 5-15ms in practice,
    // and over hundreds of chars the streaming runs ~30% over target.
    // Bo flagged this 2026-05-01: 13.4s actual vs 10s target on a
    // 361-char response. With sleep_until, drift is bounded — if one
    // emit takes 5ms longer than scheduled, the next sleep is 5ms shorter
    // because we sleep until an absolute clock time.
    //
    // The delay calc itself includes random variance (±50%) and
    // punctuation pauses (clause/sentence/paragraph). The frontend
    // pacedRenderer applies no further delays — it just appends each
    // char to pendingAssistant as events arrive.
    let stream_start = std::time::Instant::now();
    let mut next_emit_at = stream_start;
    let mut prev_char: Option<char> = None;
    for ch in full.chars() {
        // Wait until the absolute target time for this char.
        tokio::time::sleep_until(tokio::time::Instant::from_std(next_emit_at)).await;
        let mut buf = [0u8; 4];
        let s = ch.encode_utf8(&mut buf);
        let _ = app.emit("dave:token", s.to_string());
        // Schedule the NEXT emit relative to the (absolute) target, not
        // relative to "right now after this emit." This is what makes the
        // drift compensation work.
        let delay_ms = pacing.delay_for(ch, prev_char);
        next_emit_at += std::time::Duration::from_millis(delay_ms);
        prev_char = Some(ch);
    }
    let actual_stream_ms = stream_start.elapsed().as_millis() as u64;
    tracing::info!(
        "run_chat_inference_and_emit: streamed in {}ms (target was {}ms, drift {}ms)",
        actual_stream_ms,
        pacing.t_streaming_ms,
        actual_stream_ms as i64 - pacing.t_streaming_ms as i64,
    );

    let assistant_msg =
        persistence::insert_message(db, conversation_id, "assistant", &full, false)
            .await
            .map_err(|e| e.to_string())?;

    let _ = app.emit(
        "dave:stream_end",
        json!({
            "conversationId": conversation_id,
            "fullText": full,
            "messageId": assistant_msg.id,
        }),
    );

    Ok(())
}

#[tauri::command]
pub async fn start_new_conversation(state: State<'_, AppState>) -> Result<i64, String> {
    persistence::create_conversation(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn latest_or_new_conversation(state: State<'_, AppState>) -> Result<i64, String> {
    persistence::latest_or_new_conversation(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_recent_messages(
    state: State<'_, AppState>,
    conversation_id: i64,
    limit: Option<i64>,
) -> Result<Vec<Message>, String> {
    let lim = limit.unwrap_or(VIEW_LOAD_LIMIT);
    persistence::load_recent_messages(&state.db, conversation_id, lim)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_unread_journal(state: State<'_, AppState>) -> Result<Vec<JournalEntry>, String> {
    persistence::load_unread_journal(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_all_journal(state: State<'_, AppState>) -> Result<Vec<JournalEntry>, String> {
    persistence::load_all_journal(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mark_journal_surfaced(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    persistence::mark_journal_surfaced(&state.db, id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn report_user_present(state: State<'_, AppState>) -> Result<(), String> {
    persistence::touch_user_input(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn departure_entry(state: State<'_, AppState>) -> Result<Option<JournalEntry>, String> {
    persistence::latest_departure_unsurfaced(&state.db)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ensure_startup_entry(
    state: State<'_, AppState>,
) -> Result<Option<JournalEntry>, String> {
    let recent = persistence::has_recent_unsurfaced(&state.db, 12 * 3600)
        .await
        .map_err(|e| e.to_string())?;
    if recent {
        return Ok(None);
    }

    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: prompts::STARTUP_META.into(),
        },
    ];
    let content = state
        .client
        .complete(messages, 120, 0.9)
        .await
        .map_err(|e| e.to_string())?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let entry = persistence::insert_journal(&state.db, "startup", trimmed)
        .await
        .map_err(|e| e.to_string())?;
    Ok(Some(entry))
}

#[tauri::command]
pub fn buffer_size() -> i64 {
    memory_assembler::RECENT_MESSAGE_TARGET as i64
}

#[tauri::command]
pub async fn load_outreach_drops(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<OutreachDrop>, String> {
    let lim = limit.unwrap_or(100);
    persistence::load_recent_outreach_drops(&state.db, lim)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    persistence::get_setting(&state.db, &key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    persistence::set_setting(&state.db, &key, &value)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn inject_test_conversation(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<i64, String> {
    let conv_id = persistence::inject_test_conversation(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("dave:db_reset", ());
    Ok(conv_id)
}

#[tauri::command]
pub async fn clear_all_data(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    persistence::clear_all_data(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    let _ = app.emit("dave:db_reset", ());
    Ok(())
}

// ============================================================================
// Memory inspector commands (Ctrl+Shift+M panel)
// ============================================================================

#[derive(serde::Serialize)]
pub struct PartitionView {
    pub conversation_id: i64,
    pub system_prompt: String,
    pub anchor: Vec<Message>,
    pub canvas: String,
    pub middle: Vec<MiddleBlockDto>,
    pub recent: Vec<Message>,
    pub anchor_tokens: usize,
    pub canvas_tokens: usize,
    pub middle_tokens: usize,
    pub recent_tokens: usize,
    pub total_tokens: usize,
    pub token_budget_total: usize,
    pub token_reserve: usize,
    pub anchor_message_count: usize,
    pub recent_message_target: usize,
    pub recent_message_trigger: usize,
}

#[derive(serde::Serialize)]
#[serde(tag = "kind")]
pub enum MiddleBlockDto {
    #[serde(rename = "epoch")]
    Epoch { epoch: ConsolidationEpoch },
    #[serde(rename = "messages")]
    Messages { messages: Vec<Message> },
}

#[tauri::command]
pub async fn load_partition_view(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<PartitionView, String> {
    let all_msgs = persistence::load_all_messages(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let active_epochs = persistence::list_active_epochs(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let canvas = persistence::get_canvas(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let part = memory_assembler::partition(&all_msgs, &active_epochs, &canvas);

    let anchor_tokens = part.anchor_tokens();
    let canvas_tokens = part.canvas_tokens();
    let middle_tokens = part.middle_tokens();
    let recent_tokens = part.recent_tokens();
    let total_tokens = part.total_tokens();

    let middle = part.middle.into_iter().map(|b| match b {
        memory_assembler::MiddleBlock::Epoch(e) => MiddleBlockDto::Epoch { epoch: e },
        memory_assembler::MiddleBlock::Messages(ms) => MiddleBlockDto::Messages { messages: ms },
    }).collect();

    Ok(PartitionView {
        conversation_id,
        system_prompt: prompts::SYSTEM_PROMPT.to_string(),
        anchor: part.anchor,
        canvas: part.canvas,
        middle,
        recent: part.recent,
        anchor_tokens,
        canvas_tokens,
        middle_tokens,
        recent_tokens,
        total_tokens,
        token_budget_total: memory_assembler::TOKEN_BUDGET_TOTAL,
        token_reserve: memory_assembler::TOKEN_RESERVE,
        anchor_message_count: memory_assembler::ANCHOR_MESSAGE_COUNT,
        recent_message_target: memory_assembler::RECENT_MESSAGE_TARGET,
        recent_message_trigger: memory_assembler::RECENT_MESSAGE_TRIGGER,
    })
}

#[tauri::command]
pub async fn edit_message_content(
    state: State<'_, AppState>,
    conversation_id: i64,
    message_id: i64,
    new_content: String,
    reason: String,
) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("reason is required".into());
    }
    let prior = persistence::update_message_content(&state.db, message_id, &new_content)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("message {} not found", message_id))?;
    persistence::insert_memory_edit(
        &state.db,
        conversation_id,
        "message_edit",
        Some(message_id),
        Some(&prior),
        Some(&new_content),
        &reason,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn get_memory_canvas(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<String, String> {
    persistence::get_canvas(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_memory_canvas(
    state: State<'_, AppState>,
    conversation_id: i64,
    content: String,
    reason: String,
) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("reason is required".into());
    }
    let prior = persistence::set_canvas(&state.db, conversation_id, &content)
        .await
        .map_err(|e| e.to_string())?;
    persistence::insert_memory_edit(
        &state.db,
        conversation_id,
        "canvas_edit",
        None,
        Some(&prior),
        Some(&content),
        &reason,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_all_epochs_cmd(
    state: State<'_, AppState>,
    conversation_id: i64,
) -> Result<Vec<ConsolidationEpoch>, String> {
    persistence::list_all_epochs(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn edit_epoch_content(
    state: State<'_, AppState>,
    conversation_id: i64,
    epoch_id: i64,
    new_content: String,
    reason: String,
) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("reason is required".into());
    }
    let new_token_count = memory_assembler::estimate_tokens(&new_content) as i64;
    let prior = persistence::update_epoch_content(&state.db, epoch_id, &new_content, new_token_count)
        .await
        .map_err(|e| e.to_string())?;
    let prior_str = prior.unwrap_or_default();
    persistence::insert_memory_edit(
        &state.db,
        conversation_id,
        "epoch_text_edit",
        Some(epoch_id),
        Some(&prior_str),
        Some(&new_content),
        &reason,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn manual_consolidate_range(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: i64,
    range_start_message_id: i64,
    range_end_message_id: i64,
    reason: String,
) -> Result<ConsolidationEpoch, String> {
    if reason.trim().is_empty() {
        return Err("reason is required".into());
    }
    let epoch = crate::consolidation::manual_consolidate(
        &app,
        &state.db,
        &state.client,
        conversation_id,
        range_start_message_id,
        range_end_message_id,
    )
    .await
    .map_err(|e| e.to_string())?;
    persistence::insert_memory_edit(
        &state.db,
        conversation_id,
        "manual_consolidation",
        Some(epoch.id),
        None,
        Some(&epoch.content),
        &reason,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(epoch)
}

#[tauri::command]
pub async fn list_memory_edits_cmd(
    state: State<'_, AppState>,
    conversation_id: i64,
    limit: Option<i64>,
) -> Result<Vec<MemoryEdit>, String> {
    let lim = limit.unwrap_or(200);
    persistence::list_memory_edits(&state.db, conversation_id, lim)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revert_memory_edit(
    state: State<'_, AppState>,
    edit_id: i64,
    reason: String,
) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("reason is required".into());
    }
    // Load the edit row.
    let edits = persistence::list_memory_edits(&state.db, 0, 99999)
        .await
        .map_err(|e| e.to_string())?;
    // We didn't load by id; find by id. Cheaper to fetch all edits across
    // all conversations than add a dedicated by-id helper for this MVP.
    let edit = edits.into_iter().find(|e| e.id == edit_id)
        .ok_or_else(|| format!("edit {} not found", edit_id))?;

    match edit.edit_type.as_str() {
        "epoch_text_edit" => {
            let target = edit.target_id.ok_or_else(|| "missing target".to_string())?;
            let prior = edit.prior_content.unwrap_or_default();
            let toks = memory_assembler::estimate_tokens(&prior) as i64;
            persistence::update_epoch_content(&state.db, target, &prior, toks)
                .await
                .map_err(|e| e.to_string())?;
            persistence::insert_memory_edit(
                &state.db,
                edit.conversation_id,
                "epoch_text_edit",
                Some(target),
                edit.new_content.as_deref(),
                Some(&prior),
                &format!("revert of edit #{}: {}", edit_id, reason),
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        "canvas_edit" => {
            let prior = edit.prior_content.unwrap_or_default();
            let was = persistence::set_canvas(&state.db, edit.conversation_id, &prior)
                .await
                .map_err(|e| e.to_string())?;
            persistence::insert_memory_edit(
                &state.db,
                edit.conversation_id,
                "canvas_edit",
                None,
                Some(&was),
                Some(&prior),
                &format!("revert of edit #{}: {}", edit_id, reason),
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        "message_edit" => {
            let target = edit.target_id.ok_or_else(|| "missing target".to_string())?;
            let prior = edit.prior_content.unwrap_or_default();
            let was = persistence::update_message_content(&state.db, target, &prior)
                .await
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            persistence::insert_memory_edit(
                &state.db,
                edit.conversation_id,
                "message_edit",
                Some(target),
                Some(&was),
                Some(&prior),
                &format!("revert of edit #{}: {}", edit_id, reason),
            )
            .await
            .map_err(|e| e.to_string())?;
            Ok(())
        }
        other => Err(format!("revert not supported for edit_type={}", other)),
    }
}

#[tauri::command]
pub async fn export_database(app: AppHandle) -> Result<String, String> {
    use chrono::Local;
    use std::path::PathBuf;

    let project_root = match crate::sidecar::dave_data_dir(&app) {
        Ok(p) => p,
        Err(e) => return Err(format!("could not resolve data dir: {}", e)),
    };
    let src = project_root.join("dave.db");
    if !src.exists() {
        return Err(format!("dave.db not found at {}", src.display()));
    }
    let exports_dir = project_root.join("dave_exports");
    if let Err(e) = std::fs::create_dir_all(&exports_dir) {
        return Err(format!("could not create exports dir: {}", e));
    }
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dst: PathBuf = exports_dir.join(format!("dave_{}.db", stamp));
    if let Err(e) = std::fs::copy(&src, &dst) {
        return Err(format!("copy failed: {}", e));
    }
    Ok(dst.display().to_string())
}
