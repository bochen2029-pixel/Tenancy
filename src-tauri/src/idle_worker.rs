use chrono::{Local, Utc};
use rand::Rng;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio::time::{sleep, Duration};

use crate::harness;
use crate::llama_client::{ChatMessage, LlamaClient};
use crate::persistence::{self, DbHandle};
use crate::prompts;

const IDLE_THRESHOLD_SECONDS: i64 = 3 * 3600;
const MIN_TICK_SECONDS: u64 = 2 * 3600;
const MAX_TICK_SECONDS: u64 = 8 * 3600;

pub fn spawn(app: AppHandle, db: DbHandle, client: Arc<LlamaClient>) -> oneshot::Sender<()> {
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    tauri::async_runtime::spawn(async move {
        loop {
            let wait_dur = {
                let mut rng = rand::thread_rng();
                Duration::from_secs(rng.gen_range(MIN_TICK_SECONDS..MAX_TICK_SECONDS))
            };

            tokio::select! {
                _ = sleep(wait_dur) => {}
                _ = &mut shutdown_rx => {
                    tracing::info!("idle worker shutting down");
                    return;
                }
            }

            if let Err(e) = check_and_generate(&app, &db, &client).await {
                tracing::warn!("idle worker tick error: {}", e);
            }
        }
    });

    shutdown_tx
}

async fn check_and_generate(
    app: &AppHandle,
    db: &DbHandle,
    client: &LlamaClient,
) -> anyhow::Result<()> {
    let presence = persistence::get_presence(db).await?;
    let now = Utc::now().timestamp();
    let elapsed = now - presence.last_user_input;

    if elapsed < IDLE_THRESHOLD_SECONDS {
        return Ok(());
    }

    let now_local = Local::now();
    let time_str = harness::format_clock(&now_local);
    let day = now_local.format("%A").to_string();
    let date = now_local.format("%B %-d").to_string();
    let duration = harness::humanize_duration(elapsed);

    let prompt = prompts::idle_meta(&time_str, &day, &date, &duration);
    let messages = vec![
        ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        ChatMessage {
            role: "user".into(),
            content: prompt,
        },
    ];

    let content = client.complete(messages, 300, 0.95).await?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let entry = persistence::insert_journal(db, "idle", trimmed).await?;
    let _ = app.emit("dave:journal_arrived", &entry);
    tracing::info!("idle journal entry written, id={}", entry.id);
    Ok(())
}

