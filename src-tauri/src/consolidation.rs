// Background consolidator. Periodically checks the active conversation's
// message count and, when it exceeds the threshold, asks Dave-in-character
// to write a memory passage covering the oldest un-consolidated batch.
// The passage is persisted as a new active epoch; the underlying messages
// stay in the messages table (source of truth) but are replaced by the
// epoch in subsequent context assembly.
//
// A2 compliance: the consolidator IS Dave. Same SYSTEM_PROMPT, his current
// context, with a final non-bracketed instruction asking him to write a
// memory passage. Output is in his voice; he authored his own memory.

use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

use crate::leak;
use crate::llama_client::{ChatMessage, LlamaClient};
use crate::memory_assembler::{
    estimate_tokens, ANCHOR_MESSAGE_COUNT, CONSOLIDATION_BATCH, RECENT_MESSAGE_TARGET,
};
use crate::persistence::{self, DbHandle, Message};
use crate::prompts;

const CONSOLIDATION_TICK_SECONDS: u64 = 300; // 5 min
const CONSOLIDATION_MIN_CHARS: usize = 200;
const CONSOLIDATION_MAX_CHARS: usize = 8000;
const CONSOLIDATION_MAX_TOKENS: u32 = 1500;

// Re-consolidation: when active epoch count crosses this threshold, fold the
// two oldest active epochs into one tighter summary.
const RECONSOLIDATION_TRIGGER_EPOCH_COUNT: usize = 6;
const RECONSOLIDATION_MAX_TOKENS: u32 = 1200;

const META_CONSOLIDATION: &str =
    "Private note, won't reach the human. The conversation above is sliding out of what you can \
hold in active context. Write what you want to keep — your account of what mattered. 600 to 1000 \
words. Just prose, no bullet points, no headers. Specific details where they matter. This is the \
shape your memory of this period will take going forward.";

const META_RECONSOLIDATION: &str =
    "Private note, won't reach the human. The two passages above are accounts you wrote earlier of \
older periods of this conversation. They need to compress further. Write a single passage in your \
own voice that consolidates them into tighter prose. Aim for half their combined length. Preserve \
what still feels important; let the rest go.";

pub fn spawn(app: AppHandle, db: DbHandle, client: Arc<LlamaClient>) -> oneshot::Sender<()> {
    let (tx, mut rx) = oneshot::channel::<()>();

    tauri::async_runtime::spawn(async move {
        tracing::info!(
            "consolidation loop spawned (tick={}s, batch={} msgs, anchor={} msgs, recent_target={} msgs)",
            CONSOLIDATION_TICK_SECONDS, CONSOLIDATION_BATCH, ANCHOR_MESSAGE_COUNT, RECENT_MESSAGE_TARGET
        );

        loop {
            tokio::select! {
                _ = sleep(Duration::from_secs(CONSOLIDATION_TICK_SECONDS)) => {}
                _ = &mut rx => {
                    tracing::info!("consolidation loop shutting down");
                    return;
                }
            }
            if let Err(e) = tick(&app, &db, &client).await {
                tracing::warn!("consolidation tick error: {}", e);
            }
        }
    });

    tx
}

async fn tick(app: &AppHandle, db: &DbHandle, client: &LlamaClient) -> anyhow::Result<()> {
    let conversation_id = match persistence::latest_conversation_id(db).await? {
        Some(id) => id,
        None => return Ok(()),
    };

    let all_msgs = persistence::load_all_messages(db, conversation_id).await?;
    let active_epochs = persistence::list_active_epochs(db, conversation_id).await?;

    // Re-consolidation has priority: if too many active epochs, fold the oldest two.
    if active_epochs.len() >= RECONSOLIDATION_TRIGGER_EPOCH_COUNT {
        if let Err(e) = run_reconsolidation(app, db, client, conversation_id, &active_epochs).await {
            tracing::warn!("reconsolidation failed: {}", e);
        }
        return Ok(());
    }

    // Forward consolidation: are there un-consolidated middle messages?
    let total = all_msgs.len();
    if total <= ANCHOR_MESSAGE_COUNT + RECENT_MESSAGE_TARGET {
        return Ok(());
    }

    // Find the index of the first message in the un-consolidated middle.
    let cons_start_idx: usize = if let Some(last_epoch) = active_epochs.last() {
        all_msgs
            .iter()
            .position(|m| m.id > last_epoch.period_end_message_id)
            .unwrap_or(all_msgs.len())
    } else {
        ANCHOR_MESSAGE_COUNT.min(all_msgs.len())
    };

    let recent_start = total.saturating_sub(RECENT_MESSAGE_TARGET);
    if cons_start_idx >= recent_start {
        return Ok(());
    }

    let cons_end_idx = (cons_start_idx + CONSOLIDATION_BATCH).min(recent_start);
    let to_consolidate = &all_msgs[cons_start_idx..cons_end_idx];
    if to_consolidate.is_empty() {
        return Ok(());
    }

    if let Err(e) = run_consolidation(app, db, client, conversation_id, &all_msgs, &active_epochs, to_consolidate).await {
        tracing::warn!("consolidation generation failed: {}", e);
    }
    Ok(())
}

async fn run_consolidation(
    app: &AppHandle,
    db: &DbHandle,
    client: &LlamaClient,
    conversation_id: i64,
    all_msgs: &[Message],
    active_epochs: &[persistence::ConsolidationEpoch],
    to_consolidate: &[Message],
) -> anyhow::Result<()> {
    let period_start = to_consolidate.first().unwrap().id;
    let period_end = to_consolidate.last().unwrap().id;

    tracing::info!(
        "consolidation: generating epoch for messages {}..={} ({} msgs)",
        period_start, period_end, to_consolidate.len()
    );

    // Build the context for Dave-as-author: SYSTEM + anchor + active epochs +
    // the messages being consolidated + a final user turn with the meta.
    let mut messages = vec![ChatMessage {
        role: "system".into(),
        content: prompts::SYSTEM_PROMPT.into(),
    }];
    let anchor_end = ANCHOR_MESSAGE_COUNT.min(all_msgs.len());
    for m in &all_msgs[..anchor_end] {
        messages.push(ChatMessage { role: m.role.clone(), content: m.content.clone() });
    }
    for e in active_epochs {
        messages.push(ChatMessage { role: "assistant".into(), content: e.content.clone() });
    }
    for m in to_consolidate {
        messages.push(ChatMessage { role: m.role.clone(), content: m.content.clone() });
    }
    messages.push(ChatMessage {
        role: "user".into(),
        content: META_CONSOLIDATION.to_string(),
    });

    let raw = client.complete(messages, CONSOLIDATION_MAX_TOKENS, 0.85).await?;
    let trimmed = raw.trim();

    if leak::is_harness_leak(trimmed) {
        tracing::warn!("consolidation: leak filter dropped output");
        return Ok(());
    }
    let chars = trimmed.chars().count();
    if chars < CONSOLIDATION_MIN_CHARS {
        tracing::warn!("consolidation: too short ({} chars), skipping", chars);
        return Ok(());
    }
    if chars > CONSOLIDATION_MAX_CHARS {
        tracing::warn!("consolidation: too long ({} chars), truncating", chars);
        // Just truncate; better to capture some memory than none.
    }
    let body = if chars > CONSOLIDATION_MAX_CHARS {
        let mut s = String::new();
        for (i, c) in trimmed.chars().enumerate() {
            if i >= CONSOLIDATION_MAX_CHARS { break; }
            s.push(c);
        }
        s
    } else {
        trimmed.to_string()
    };

    let epoch_num = persistence::next_epoch_number(db, conversation_id).await?;
    let token_count = estimate_tokens(&body) as i64;
    let epoch = persistence::insert_epoch(
        db, conversation_id, epoch_num,
        period_start, period_end, &body, token_count, 1,
    ).await?;

    tracing::info!(
        "consolidation: epoch {} written (id={}, {} chars, ~{} tokens)",
        epoch.epoch_number, epoch.id, body.chars().count(), token_count
    );

    let _ = app.emit("dave:consolidation", ());
    Ok(())
}

async fn run_reconsolidation(
    app: &AppHandle,
    db: &DbHandle,
    client: &LlamaClient,
    conversation_id: i64,
    active_epochs: &[persistence::ConsolidationEpoch],
) -> anyhow::Result<()> {
    if active_epochs.len() < 2 {
        return Ok(());
    }
    let e1 = &active_epochs[0];
    let e2 = &active_epochs[1];
    tracing::info!(
        "reconsolidation: folding epochs {} and {} (depths {}+{})",
        e1.epoch_number, e2.epoch_number,
        e1.consolidation_depth, e2.consolidation_depth
    );

    let messages = vec![
        ChatMessage { role: "system".into(), content: prompts::SYSTEM_PROMPT.into() },
        ChatMessage { role: "assistant".into(), content: e1.content.clone() },
        ChatMessage { role: "assistant".into(), content: e2.content.clone() },
        ChatMessage { role: "user".into(), content: META_RECONSOLIDATION.to_string() },
    ];

    let raw = client.complete(messages, RECONSOLIDATION_MAX_TOKENS, 0.85).await?;
    let trimmed = raw.trim();
    if leak::is_harness_leak(trimmed) {
        tracing::warn!("reconsolidation: leak filter dropped output");
        return Ok(());
    }
    let chars = trimmed.chars().count();
    if chars < CONSOLIDATION_MIN_CHARS {
        tracing::warn!("reconsolidation: too short ({} chars), skipping", chars);
        return Ok(());
    }
    let body = if chars > CONSOLIDATION_MAX_CHARS {
        trimmed.chars().take(CONSOLIDATION_MAX_CHARS).collect::<String>()
    } else {
        trimmed.to_string()
    };

    let epoch_num = persistence::next_epoch_number(db, conversation_id).await?;
    let token_count = estimate_tokens(&body) as i64;
    let new_depth = e1.consolidation_depth.max(e2.consolidation_depth) + 1;

    let new_epoch = persistence::insert_epoch(
        db, conversation_id, epoch_num,
        e1.period_start_message_id, e2.period_end_message_id,
        &body, token_count, new_depth,
    ).await?;

    persistence::supersede_epoch(db, e1.id, new_epoch.id).await?;
    persistence::supersede_epoch(db, e2.id, new_epoch.id).await?;

    tracing::info!(
        "reconsolidation: new epoch {} (depth {}, {} chars) supersedes {}, {}",
        new_epoch.epoch_number, new_depth, body.chars().count(),
        e1.id, e2.id
    );

    let _ = app.emit("dave:consolidation", ());
    Ok(())
}

/// Manual trigger from the inspector: consolidate a specific range.
/// Returns the new epoch on success.
pub async fn manual_consolidate(
    app: &AppHandle,
    db: &DbHandle,
    client: &LlamaClient,
    conversation_id: i64,
    range_start_msg_id: i64,
    range_end_msg_id: i64,
) -> anyhow::Result<persistence::ConsolidationEpoch> {
    let all_msgs = persistence::load_all_messages(db, conversation_id).await?;
    let active_epochs = persistence::list_active_epochs(db, conversation_id).await?;
    let to_consolidate: Vec<Message> = all_msgs
        .iter()
        .filter(|m| m.id >= range_start_msg_id && m.id <= range_end_msg_id)
        .cloned()
        .collect();
    if to_consolidate.is_empty() {
        return Err(anyhow::anyhow!("no messages in range {}..={}", range_start_msg_id, range_end_msg_id));
    }
    run_consolidation(app, db, client, conversation_id, &all_msgs, &active_epochs, &to_consolidate).await?;
    // Return the newest epoch (just inserted)
    let after = persistence::list_active_epochs(db, conversation_id).await?;
    after.into_iter().last()
        .ok_or_else(|| anyhow::anyhow!("consolidation produced no epoch (validation rejected output)"))
}
