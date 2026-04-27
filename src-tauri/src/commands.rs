use serde_json::json;
use tauri::{AppHandle, Emitter, State};

use chrono::Local;

use crate::leak;
use crate::llama_client::ChatMessage;
use crate::memory_assembler;
use crate::persistence::{self, ConsolidationEpoch, JournalEntry, MemoryEdit, Message, OutreachDrop};
use crate::prompts;
use crate::time_awareness;
use crate::AppState;

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

    // Assemble context with memory partition: anchor + active epochs +
    // un-consolidated middle + recent. Replaces the old flat HISTORY_BUFFER_SIZE
    // approach.
    let all_msgs = persistence::load_all_messages(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let active_epochs = persistence::list_active_epochs(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let canvas = persistence::get_canvas(&state.db, conversation_id)
        .await
        .map_err(|e| e.to_string())?;
    let partition = memory_assembler::partition(&all_msgs, &active_epochs, &canvas);

    // Conditional time-awareness: if the user's message reaches for time,
    // prepend a single ambient sentence to the system prompt for THIS request
    // only. Persistent context stays clean. Dave only knows the time when
    // the human is invoking it.
    let system_content = if time_awareness::user_message_invokes_time(&user_text) {
        let now = Local::now();
        time_awareness::system_prompt_with_time(prompts::SYSTEM_PROMPT, &now)
    } else {
        prompts::SYSTEM_PROMPT.to_string()
    };

    let messages = memory_assembler::build_chat_messages(
        &system_content,
        &partition,
        Some(&user_text),
    );

    let user_msg =
        persistence::insert_message(&state.db, conversation_id, "user", &user_text, false)
            .await
            .map_err(|e| e.to_string())?;
    let _ = app.emit("dave:user_persisted", &user_msg);

    let _ = app.emit("dave:stream_start", ());

    let app_for_token = app.clone();
    let full = state
        .client
        .chat_stream(messages, move |tok| {
            let _ = app_for_token.emit("dave:token", tok);
        })
        .await
        .map_err(|e| e.to_string())?;

    // Defense in depth: if Dave's response is a harness leak, drop it.
    // The new persona prompt should never produce [pass]/[meta]/etc., but
    // if regression ever puts those tokens back into his vocabulary this
    // filter catches it before it reaches persistence or the user.
    if leak::is_harness_leak(&full) {
        tracing::warn!("send_to_dave: dropping harness leak from response");
        let _ = app.emit("dave:stream_aborted", ());
        return Ok(());
    }

    let assistant_msg =
        persistence::insert_message(&state.db, conversation_id, "assistant", &full, false)
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
