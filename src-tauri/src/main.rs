#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod chat_pacing;
mod chat_triage;
mod commands;
mod consolidation;
mod discriminator;
mod harness;
mod headless;
mod idle_worker;
mod leak;
mod llama_client;
mod memory_assembler;
mod outreach;
mod persistence;
mod presence;
mod prompts;
mod sidecar;
mod think_strip;
mod time_awareness;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
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
    /// Live system prompt cache. Initialized at startup from the
    /// `active_system_prompt` DB setting (or `prompts::SYSTEM_PROMPT` when
    /// empty/missing) and hot-swappable via the persona commands. Every
    /// callsite that previously referenced `prompts::SYSTEM_PROMPT`
    /// directly now read-locks this and clones the current value. Workers
    /// (idle, outreach, consolidation) hold their own `Arc` to the same
    /// `RwLock`, so swaps propagate to them without a respawn.
    pub system_prompt: Arc<RwLock<String>>,
    /// Live window-focus flag, updated on WindowEvent::Focused. Read by the
    /// presence sensor to distinguish "in the chat" (focused) from
    /// "present-but-elsewhere" (unfocused + recent OS input).
    pub window_focused: Arc<AtomicBool>,
}

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("dave=info,llama=info")),
        )
        .try_init();

    // Headless "sit with Dave" harness — reproduces the real chat pipeline
    // (persona + memory partition + model) without the webview. Used for
    // testing Dave's mind headlessly. Assumes llama-server is already up.
    if std::env::var("DAVE_HEADLESS").is_ok() {
        headless::run();
        return;
    }

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
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                let app = window.app_handle().clone();
                api.prevent_close();
                let _ = window.hide();
                tauri::async_runtime::spawn(async move {
                    handle_close(app).await;
                });
            }
            WindowEvent::Focused(focused) => {
                // Presence signal: is the user in Dave's window right now?
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    state.window_focused.store(*focused, Ordering::Relaxed);
                }
            }
            _ => {}
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
            commands::list_models,
            commands::get_active_model,
            commands::get_thinking_enabled,
            commands::set_thinking_enabled,
            commands::switch_model,
            commands::list_personas,
            commands::get_default_system_prompt,
            commands::load_persona_text,
            commands::get_system_prompt,
            commands::set_system_prompt,
            commands::reset_system_prompt,
            commands::rate_last_reach,
            commands::mark_missed_reach,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn init(app: AppHandle) -> anyhow::Result<()> {
    // DB has to come up before the sidecar now — the sidecar reads its
    // active-model-path + thinking-enabled settings from the DB on spawn.
    let data_dir = sidecar::dave_data_dir(&app)?;
    let db_path = data_dir.join("dave.db");
    let db = persistence::open(&db_path)?;
    persistence::touch_app_open(&db).await?;

    // Rotating disaster-recovery backup of the DB (keeps the last few snapshots
    // in <data_dir>/backups/ via VACUUM INTO — each a fully-checkpointed single
    // file). Best-effort, runs once before the session writes anything, so a
    // corruption or accidental wipe never costs more than the last session.
    persistence::rotate_backup(&db, &data_dir);

    // Seed the portable personas dir (release: %LOCALAPPDATA%\...\personas)
    // with an editable example so preset personas work on a bare machine.
    prompts::seed_personas();

    // Boot-time GGUF inventory — logs every available model so we can
    // confirm in dev.log that the running binary sees C:\models. If
    // the dropdown is later empty, this log line tells us whether the
    // problem is path/permissions (here) or IPC plumbing (frontend).
    let inventory = sidecar::list_available_models();
    tracing::info!("boot inventory: {} GGUF(s) discoverable", inventory.len());
    for p in &inventory {
        tracing::info!("  GGUF: {}", p.display());
    }

    let child = sidecar::spawn_llama_server(&app, &db).await?;

    let client = Arc::new(llama_client::LlamaClient::new("http://127.0.0.1:8080"));

    let chat_in_flight = Arc::new(AtomicBool::new(false));
    // Window starts focused (Tauri shows it foreground on launch). Updated by
    // the WindowEvent::Focused handler; read by the presence sensor.
    let window_focused = Arc::new(AtomicBool::new(true));

    // Resolve the live system prompt now (DB setting overrides the in-binary
    // default). One-time read at boot; after this, every read goes through
    // the Arc<RwLock<String>> below — no DB hit on the hot path. Logged so
    // dev.log shows whether the active prompt is the built-in or a swap.
    let initial_prompt = prompts::resolve_active_system_prompt(&db).await;
    let prompt_source = if initial_prompt == prompts::SYSTEM_PROMPT {
        "built-in default"
    } else {
        "DB override"
    };
    tracing::info!(
        "boot persona: {} ({} chars)",
        prompt_source,
        initial_prompt.chars().count()
    );
    let system_prompt = Arc::new(RwLock::new(initial_prompt));

    let idle_tx = idle_worker::spawn(
        app.clone(),
        db.clone(),
        client.clone(),
        system_prompt.clone(),
    );
    let outreach_tx = outreach::spawn(
        app.clone(),
        db.clone(),
        client.clone(),
        chat_in_flight.clone(),
        system_prompt.clone(),
        window_focused.clone(),
    );
    // Presence sensor: logs the user-presence timeline (in_chat /
    // present_elsewhere / away) for the initiation-timing corpus. Sense-only.
    presence::spawn_sampler(app.clone(), db.clone(), window_focused.clone());
    let consolidation_tx = consolidation::spawn(
        app.clone(),
        db.clone(),
        client.clone(),
        system_prompt.clone(),
    );

    app.manage(AppState {
        db,
        client,
        llama_child: Mutex::new(Some(child)),
        idle_shutdown: Mutex::new(Some(idle_tx)),
        outreach_shutdown: Mutex::new(Some(outreach_tx)),
        consolidation_shutdown: Mutex::new(Some(consolidation_tx)),
        chat_in_flight,
        system_prompt,
        window_focused,
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
        // Snapshot the live persona so the departure ritual fires with
        // whatever the operator has selected, not the in-binary default.
        let sys_prompt = state.system_prompt.read().unwrap().clone();
        (
            state.client.clone(),
            state.db.clone(),
            idle,
            outreach,
            consolidation,
            child,
            sys_prompt,
        )
    };
    let (client, db, idle_shutdown, outreach_shutdown, consolidation_shutdown, llama_child, sys_prompt) = extracted;

    let messages = vec![
        llama_client::ChatMessage {
            role: "system".into(),
            content: sys_prompt,
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

    // Fold the WAL back into dave.db so the main file is self-contained on
    // disk after a clean close (makes external copies / the next backup whole).
    persistence::checkpoint_wal(&db);

    app.exit(0);
}
