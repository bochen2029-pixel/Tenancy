#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod consolidation;
mod discriminator;
mod harness;
mod idle_worker;
mod leak;
mod llama_client;
mod memory_assembler;
mod outreach;
mod persistence;
mod prompts;
mod sidecar;
mod think_strip;
mod time_awareness;

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tokio::process::Child;
use tokio::sync::oneshot;
use tracing_subscriber::EnvFilter;

pub struct AppState {
    pub db: persistence::DbHandle,
    pub client: Arc<llama_client::LlamaClient>,
    pub llama_child: Mutex<Option<Child>>,
    pub idle_shutdown: Mutex<Option<oneshot::Sender<()>>>,
    pub outreach_shutdown: Mutex<Option<oneshot::Sender<()>>>,
    pub consolidation_shutdown: Mutex<Option<oneshot::Sender<()>>>,
    /// True while the chat path (send_to_dave) is actively streaming a reply
    /// to the frontend. Outreach checks this flag both before starting its
    /// (multi-sample) inference and again before emission, to prevent
    /// concurrent streams from racing on the frontend's pacedRenderer.
    /// Without this, an outreach fire that completes while the chat path is
    /// mid-stream produces interleaved garbage in the conversation pane.
    pub chat_in_flight: Arc<AtomicBool>,
}

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dave=info,llama=info")),
        )
        .try_init();

    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = init(handle.clone()).await {
                    tracing::error!("init failed: {}", e);
                    let _ = handle.emit("dave:init_error", e.to_string());
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle().clone();
                api.prevent_close();
                let _ = window.hide();
                tauri::async_runtime::spawn(async move {
                    handle_close(app).await;
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_to_dave,
            commands::start_new_conversation,
            commands::latest_or_new_conversation,
            commands::load_recent_messages,
            commands::load_unread_journal,
            commands::load_all_journal,
            commands::mark_journal_surfaced,
            commands::report_user_present,
            commands::departure_entry,
            commands::ensure_startup_entry,
            commands::buffer_size,
            commands::load_outreach_drops,
            commands::get_setting,
            commands::set_setting,
            commands::inject_test_conversation,
            commands::clear_all_data,
            commands::export_database,
            commands::load_partition_view,
            commands::list_all_epochs_cmd,
            commands::edit_epoch_content,
            commands::manual_consolidate_range,
            commands::list_memory_edits_cmd,
            commands::revert_memory_edit,
            commands::get_memory_canvas,
            commands::set_memory_canvas,
            commands::edit_message_content,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn init(app: AppHandle) -> anyhow::Result<()> {
    let child = sidecar::spawn_llama_server(&app).await?;

    let db_path = sidecar::dave_data_dir(&app)?.join("dave.db");
    let db = persistence::open(&db_path)?;
    persistence::touch_app_open(&db).await?;

    let client = Arc::new(llama_client::LlamaClient::new("http://127.0.0.1:8080"));

    let chat_in_flight = Arc::new(AtomicBool::new(false));

    let idle_tx = idle_worker::spawn(app.clone(), db.clone(), client.clone());
    let outreach_tx = outreach::spawn(
        app.clone(),
        db.clone(),
        client.clone(),
        chat_in_flight.clone(),
    );
    let consolidation_tx = consolidation::spawn(app.clone(), db.clone(), client.clone());

    app.manage(AppState {
        db,
        client,
        llama_child: Mutex::new(Some(child)),
        idle_shutdown: Mutex::new(Some(idle_tx)),
        outreach_shutdown: Mutex::new(Some(outreach_tx)),
        consolidation_shutdown: Mutex::new(Some(consolidation_tx)),
        chat_in_flight,
    });

    let _ = app.emit("dave:ready", ());
    Ok(())
}

async fn handle_close(app: AppHandle) {
    let extracted = {
        let Some(state) = app.try_state::<AppState>() else {
            app.exit(0);
            return;
        };
        let idle = state.idle_shutdown.lock().unwrap().take();
        let outreach = state.outreach_shutdown.lock().unwrap().take();
        let consolidation = state.consolidation_shutdown.lock().unwrap().take();
        let child = state.llama_child.lock().unwrap().take();
        (state.client.clone(), state.db.clone(), idle, outreach, consolidation, child)
    };
    let (client, db, idle_shutdown, outreach_shutdown, consolidation_shutdown, llama_child) = extracted;

    let messages = vec![
        llama_client::ChatMessage {
            role: "system".into(),
            content: prompts::SYSTEM_PROMPT.into(),
        },
        llama_client::ChatMessage {
            role: "user".into(),
            content: prompts::DEPARTURE_META.into(),
        },
    ];

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        client.complete(messages, 80, 0.85),
    )
    .await;

    if let Ok(Ok(content)) = result {
        let trimmed = content.trim();
        if !trimmed.is_empty() {
            let _ = persistence::insert_journal(&db, "departure", trimmed).await;
        }
    }

    let _ = persistence::touch_app_close(&db).await;

    if let Some(tx) = idle_shutdown {
        let _ = tx.send(());
    }
    if let Some(tx) = outreach_shutdown {
        let _ = tx.send(());
    }
    if let Some(tx) = consolidation_shutdown {
        let _ = tx.send(());
    }

    if let Some(mut child) = llama_child {
        let _ = child.start_kill();
    }

    app.exit(0);
}
