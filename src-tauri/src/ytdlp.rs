use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub const DOWNLOAD_EVENT: &str = "download-progress";

const PROGRESS_TEMPLATE: &str =
    "download:__DL__%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.speed)s|%(progress.eta)s|%(progress.percent)s";
const FILE_TEMPLATE: &str = "after_move:__FILE__%(filepath)s";

fn ytdlp_bin() -> String {
    std::env::var("DOWNLINK_YTDLP").unwrap_or_else(|_| "yt-dlp".to_string())
}

fn opt_f64(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() || value == "NA" || value == "None" {
        return None;
    }
    value.parse().ok()
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInfo {
    pub title: String,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub extractor: Option<String>,
}

/// Consulta los metadatos de un enlace sin descargar nada.
pub async fn probe(url: &str) -> Result<ProbeInfo, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(45),
        Command::new(ytdlp_bin())
            .args([
                "--ignore-config",
                "--skip-download",
                "--no-playlist",
                "--no-warnings",
                "--dump-single-json",
                url,
            ])
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "El sitio no respondió a tiempo. Intenta de nuevo.".to_string())?
    .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(first_error_line(&stderr)
            .unwrap_or_else(|| "No se pudo analizar el enlace.".to_string()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "yt-dlp devolvió una respuesta inválida.".to_string())?;

    Ok(ProbeInfo {
        title: json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Sin título")
            .to_string(),
        channel: json
            .get("channel")
            .or_else(|| json.get("uploader"))
            .and_then(|v| v.as_str())
            .map(String::from),
        duration: json.get("duration").and_then(|v| v.as_f64()),
        thumbnail: json
            .get("thumbnail")
            .and_then(|v| v.as_str())
            .map(String::from),
        webpage_url: json
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .unwrap_or(url)
            .to_string(),
        extractor: json
            .get("extractor_key")
            .or_else(|| json.get("extractor"))
            .and_then(|v| v.as_str())
            .map(String::from),
    })
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub job_id: u64,
    pub percent: Option<f64>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub status: &'static str,
    pub file_path: Option<String>,
    pub message: Option<String>,
}

/// Lanza la descarga en segundo plano y emite eventos `download-progress`.
pub async fn spawn_download(
    app: AppHandle,
    job_id: u64,
    url: String,
    out_dir: PathBuf,
    jobs: Arc<Mutex<HashMap>>,
) -> Result<(), String> {
    let mut child = Command::new(ytdlp_bin())
        .args([
            "--ignore-config",
            "--no-playlist",
            "--no-warnings",
            "--newline",
            "--progress",
            "--progress-template",
            PROGRESS_TEMPLATE,
            "--print",
            FILE_TEMPLATE,
            "--output",
            "%(title)s [%(id)s].%(ext)s",
            &url,
        ])
        .current_dir(&out_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "No se pudo leer la salida de yt-dlp.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "No se pudo leer el error de yt-dlp.".to_string())?;

    jobs.lock().await.insert(job_id, child);

    tauri::async_runtime::spawn(async move {
        let stderr_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let stderr_buf_task = stderr_buf.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let mut buf = stderr_buf_task.lock().await;
                buf.push_str(&line);
                buf.push('\n');
            }
        });

        let mut final_path: Option<String> = None;
        let mut lines = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(rest) = line.strip_prefix("__DL__") {
                let parts: Vec<&str> = rest.split('|').collect();
                if parts.len() == 5 {
                    let payload = ProgressPayload {
                        job_id,
                        downloaded_bytes: opt_f64(parts[0]).map(|v| v as u64),
                        total_bytes: opt_f64(parts[1]).map(|v| v as u64),
                        speed_bps: opt_f64(parts[2]),
                        eta_secs: opt_f64(parts[3]).map(|v| v as u64),
                        percent: opt_f64(parts[4]),
                        status: "downloading",
                        file_path: None,
                        message: None,
                    };
                    let _ = app.emit(DOWNLOAD_EVENT, payload);
                }
            } else if let Some(path) = line.strip_prefix("__FILE__") {
                final_path = Some(path.trim().to_string());
            }
        }

        // El proceso terminó (o lo mató cancel_download, que ya emitió su evento)
        let mut map = jobs.lock().await;
        let Some(mut child) = map.remove(&job_id) else {
            return;
        };

        let succeeded = child.try_wait().ok().flatten().is_some_and(|s| s.success());
        let stderr_text = stderr_buf.lock().await.clone();

        let payload = if succeeded {
            ProgressPayload {
                job_id,
                percent: Some(100.0),
                downloaded_bytes: None,
                total_bytes: None,
                speed_bps: None,
                eta_secs: None,
                status: "done",
                file_path: final_path,
                message: None,
            }
        } else {
            ProgressPayload {
                job_id,
                percent: None,
                downloaded_bytes: None,
                total_bytes: None,
                speed_bps: None,
                eta_secs: None,
                status: "error",
                file_path: None,
                message: Some(
                    first_error_line(&stderr_text)
                        .unwrap_or_else(|| "La descarga falló.".to_string()),
                ),
            }
        };
        let _ = app.emit(DOWNLOAD_EVENT, payload);
    });

    Ok(())
}

/// Detiene una descarga en curso. Devuelve `true` si había algo que cancelar.
pub async fn cancel(jobs: &Arc<Mutex<HashMap>>, job_id: u64) -> bool {
    let mut map = jobs.lock().await;
    if let Some(mut child) = map.remove(&job_id) {
        let _ = child.kill().await;
        true
    } else {
        false
    }
}

pub type HashMap = std::collections::HashMap<u64, Child>;

fn first_error_line(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .next_back()
        .map(String::from)
}
