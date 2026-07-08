use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

// llama-server context window in tokens. The model supports up to 262144,
// but VRAM caps it. KV cache for Qwen3.5-9B is ~45 KiB per token at fp16,
// so 65536 ~= 2.9 GiB on top of the ~5 GiB model. Headroom on 16 GiB cards
// is comfortable. Bump toward 100000 if you want more; flip down if VRAM
// pressure shows up. Frontend's HISTORY_BUFFER_SIZE in commands.rs caps
// how much of this Dave actually receives per turn.
const CTX_SIZE: u32 = 65536;

/// Settings key — currently selected model file (absolute path).
/// Persisted via the same SQLite settings table the rest of the app uses.
/// Empty / unset / missing-file all fall back to the candidate-list logic
/// in `default_model_path()`.
pub const SETTING_KEY_MODEL_PATH: &str = "active_model_path";

/// Settings key — whether thinking is enabled in the chat template.
/// Stored as "1" / "0". Default OFF: Dave wants plain replies, and any
/// thinking the model does is emitted inline as <think>…</think> (see
/// --reasoning-format none below) then stripped by ThinkStripper. Turn ON
/// via the Settings toggle to let the interactive chat path surface the
/// model reasoning-then-answer.
pub const SETTING_KEY_THINKING_ENABLED: &str = "model_thinking_enabled";

/// Directory where Dave looks for available models when populating the
/// settings dropdown. Files matching *.gguf are surfaced to the UI.
pub const MODELS_DIR: &str = r"C:\models";

pub fn dave_data_dir(app: &AppHandle) -> Result<PathBuf> {
    if cfg!(debug_assertions) {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let project_root = std::path::Path::new(manifest_dir)
            .parent()
            .ok_or_else(|| anyhow!("no parent of CARGO_MANIFEST_DIR"))?;
        Ok(project_root.to_path_buf())
    } else {
        Ok(app.path().app_local_data_dir()?)
    }
}

fn project_root() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest)
        .parent()
        .unwrap_or_else(|| std::path::Path::new(manifest))
        .to_path_buf()
}

fn llama_server_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("DAVE_LLAMA_SERVER") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(anyhow!(
            "DAVE_LLAMA_SERVER set to {} but file does not exist",
            pb.display()
        ));
    }
    let candidates = [
        PathBuf::from(r"C:\llama.cpp\llama-server.exe"),
        PathBuf::from(r"C:\Program Files\llama.cpp\llama-server.exe"),
        project_root().join("src-tauri").join("binaries").join("llama-server.exe"),
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    Err(anyhow!(
        "llama-server.exe not found. Set DAVE_LLAMA_SERVER or place at C:\\llama.cpp\\llama-server.exe"
    ))
}

/// Default model path, used when no setting has been saved or the saved
/// path no longer exists. First-found wins.
pub fn default_model_path() -> Result<PathBuf> {
    if let Ok(p) = std::env::var("DAVE_MODEL_PATH") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Ok(pb);
        }
        return Err(anyhow!(
            "DAVE_MODEL_PATH set to {} but file does not exist",
            pb.display()
        ));
    }
    let candidates = [
        project_root().join("models").join("dave.gguf"),
        PathBuf::from(r"C:\models\dave.gguf"),
        PathBuf::from(r"C:\models\Qwen3.5-9B-Q4_K_M.gguf"),
        PathBuf::from(r"C:\models\Qwen3.5-9B-Q5_K_M.gguf"),
        PathBuf::from(r"C:\models\Qwen3.5-9B-Instruct-Q4_K_M.gguf"),
        PathBuf::from(r"C:\models\Qwen3.5-9B-Instruct-Q5_K_M.gguf"),
        PathBuf::from(r"C:\models\Qwen3.5-4B-Q4_K_M.gguf"),
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    // Portability fallback: no candidate name matched. Take the first usable
    // *.gguf actually present in C:\models (or the project models dir) so the
    // app boots on a fresh machine whose model has any name. Skip auxiliary
    // GGUFs that are not standalone chat models — the vision projector
    // (mmproj), rerankers, and ggml-prefixed side files.
    for p in list_available_models() {
        let name = p
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if name.contains("mmproj") || name.contains("reranker") || name.starts_with("ggml-") {
            continue;
        }
        tracing::warn!(
            "no candidate-name match; falling back to first GGUF found: {}",
            p.display()
        );
        return Ok(p);
    }
    Err(anyhow!(
        "no GGUF model found. Place a *.gguf in C:\\models (or set DAVE_MODEL_PATH)"
    ))
}

/// Surface available models for the settings UI. Returns absolute paths to
/// every *.gguf file in MODELS_DIR plus the project's `models/` dir.
/// Sorted alphabetically. Empty list if neither dir exists.
///
/// Logs every directory scanned + every file considered, so when the UI
/// shows "no GGUFs found" you can `tail -f dev.log` to see exactly which
/// dirs the running binary actually checked. Surfaces permission denied,
/// missing dirs, and extension-mismatch all explicitly.
pub fn list_available_models() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let dirs = [
        PathBuf::from(MODELS_DIR),
        project_root().join("models"),
    ];
    for d in &dirs {
        tracing::info!("list_available_models: scanning {}", d.display());
        match std::fs::read_dir(d) {
            Ok(entries) => {
                let mut count = 0usize;
                for e in entries.flatten() {
                    count += 1;
                    let p = e.path();
                    let ext = p.extension()
                        .and_then(|s| s.to_str())
                        .map(str::to_ascii_lowercase);
                    let is_gguf = ext.as_deref() == Some("gguf");
                    tracing::debug!(
                        "  entry {} ext={:?} match={}",
                        p.display(), ext, is_gguf
                    );
                    if is_gguf && seen.insert(p.clone()) {
                        out.push(p);
                    }
                }
                tracing::info!("list_available_models:   {} entries scanned in {}", count, d.display());
            }
            Err(e) => {
                tracing::warn!(
                    "list_available_models: cannot read {}: {} (kind={:?})",
                    d.display(), e, e.kind()
                );
            }
        }
    }
    out.sort();
    tracing::info!("list_available_models: returning {} GGUFs", out.len());
    for p in &out {
        tracing::info!("  -> {}", p.display());
    }
    out
}

/// Read the active-model-path setting from the SQLite settings table.
/// Returns the saved path if it exists on disk, otherwise None (caller
/// should fall back to `default_model_path()`).
pub async fn active_model_from_settings(
    db: &crate::persistence::DbHandle,
) -> Option<PathBuf> {
    match crate::persistence::get_setting(db, SETTING_KEY_MODEL_PATH).await {
        Ok(Some(p)) if !p.is_empty() => {
            let pb = PathBuf::from(p);
            if pb.exists() {
                Some(pb)
            } else {
                tracing::warn!(
                    "saved active_model_path no longer exists: {}",
                    pb.display()
                );
                None
            }
        }
        _ => None,
    }
}

/// Read the thinking-enabled setting. Defaults FALSE when missing or
/// unparseable (Dave's design wants plain replies; see the const doc above).
pub async fn thinking_enabled_from_settings(
    db: &crate::persistence::DbHandle,
) -> bool {
    match crate::persistence::get_setting(db, SETTING_KEY_THINKING_ENABLED).await {
        Ok(Some(v)) => matches!(v.as_str(), "1" | "true" | "TRUE"),
        _ => false,
    }
}

/// Resolve the model path to use for a fresh spawn: saved setting first,
/// then default. The result is guaranteed to point to an existing file.
pub async fn resolve_model_path(db: &crate::persistence::DbHandle) -> Result<PathBuf> {
    if let Some(p) = active_model_from_settings(db).await {
        return Ok(p);
    }
    default_model_path()
}

/// Spawn llama-server using whatever model is currently active per settings
/// (or the default if no setting saved). This is the entry point used by
/// `init()` at startup.
pub async fn spawn_llama_server(
    _app: &AppHandle,
    db: &crate::persistence::DbHandle,
) -> Result<Child> {
    let model = resolve_model_path(db).await?;
    // Self-heal the persisted model path: if the saved active_model_path was
    // blank or pointed at a file that no longer exists (so resolve fell back
    // to a default), write the resolved path back. Keeps the Settings
    // dropdown's "active" marker honest instead of naming a missing file.
    let resolved = model.to_string_lossy().to_string();
    let saved = active_model_from_settings(db)
        .await
        .map(|p| p.to_string_lossy().to_string());
    if saved.as_deref() != Some(resolved.as_str()) {
        let _ = crate::persistence::set_setting(db, SETTING_KEY_MODEL_PATH, &resolved).await;
    }
    let thinking = thinking_enabled_from_settings(db).await;
    spawn_llama_server_with(_app, &model, thinking).await
}

/// Lower-level spawn used by both startup and the model-switch command.
/// Caller passes an explicit model path + thinking preference.
pub async fn spawn_llama_server_with(
    _app: &AppHandle,
    model: &Path,
    enable_thinking: bool,
) -> Result<Child> {
    let server = llama_server_path()?;
    let server_dir = server
        .parent()
        .ok_or_else(|| anyhow!("llama-server has no parent directory"))?;

    if !model.exists() {
        return Err(anyhow!("model file does not exist: {}", model.display()));
    }

    tracing::info!("llama-server: {}", server.display());
    tracing::info!("model: {}", model.display());
    tracing::info!("cwd: {}", server_dir.display());
    tracing::info!("enable_thinking: {}", enable_thinking);

    let ctx_str = CTX_SIZE.to_string();
    let chat_kwargs = format!(r#"{{"enable_thinking":{}}}"#, enable_thinking);

    let mut cmd = Command::new(&server);
    cmd.current_dir(server_dir)
        .args([
            "--model",
            &model.to_string_lossy(),
            "--ctx-size",
            &ctx_str,
            "--n-gpu-layers",
            "99",
            "--port",
            "8080",
            "--host",
            "127.0.0.1",
            "--temp",
            "0.85",
            "--top-p",
            "0.9",
            "--top-k",
            "20",
            "--repeat-penalty",
            "1.0",
            "--presence-penalty",
            "1.5",
            // Thinking-mode plumbing. --jinja activates the model's bundled
            // chat template so --chat-template-kwargs enable_thinking is
            // actually honored (the Settings toggle drives it).
            //
            // --reasoning-format none is LOAD-BEARING. It keeps any <think>…
            // </think> reasoning INLINE in delta.content instead of routing it
            // into a separate `reasoning_content` field. The SSE parser
            // (llama_client.rs) reads only delta.content, and ThinkStripper
            // removes the <think> block at the boundary. The previous value,
            // `deepseek`, sent reasoning to reasoning_content — the parser saw
            // an empty content stream and Dave produced EMPTY responses the
            // moment the model engaged thinking. `none` + ThinkStripper is
            // robust whether or not the model honors enable_thinking.
            "--jinja",
            "--chat-template-kwargs",
            &chat_kwargs,
            "--reasoning-format",
            "none",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        anyhow!("failed to spawn llama-server at {}: {}", server.display(), e)
    })?;

    if let Some(stdout) = child.stdout.take() {
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(target: "llama", "{}", line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::info!(target: "llama", "{}", line);
            }
        });
    }

    wait_for_ready().await?;
    Ok(child)
}

async fn wait_for_ready() -> Result<()> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(180);
    let url = "http://127.0.0.1:8080/health";
    loop {
        if Instant::now() > deadline {
            return Err(anyhow!("llama-server did not become ready within 180s"));
        }
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                tracing::info!("llama-server ready");
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
