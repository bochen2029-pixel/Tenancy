use anyhow::{anyhow, Result};
use std::path::PathBuf;
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

fn model_path() -> Result<PathBuf> {
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
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    Err(anyhow!(
        "no GGUF model found. Set DAVE_MODEL_PATH, or place at models/dave.gguf or C:\\models\\dave.gguf"
    ))
}

pub async fn spawn_llama_server(_app: &AppHandle) -> Result<Child> {
    let server = llama_server_path()?;
    let model = model_path()?;
    let server_dir = server
        .parent()
        .ok_or_else(|| anyhow!("llama-server has no parent directory"))?;

    tracing::info!("llama-server: {}", server.display());
    tracing::info!("model: {}", model.display());
    tracing::info!("cwd: {}", server_dir.display());

    let ctx_str = CTX_SIZE.to_string();
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
