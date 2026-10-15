use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::settings::{CookiesMode, EngineInfo, Settings};

pub const DOWNLOAD_EVENT: &str = "download-progress";

const PROGRESS_TEMPLATE: &str = "download:__DL__%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.speed)s|%(progress.eta)s|%(progress.percent)s";
const FILE_TEMPLATE: &str = "after_move:__FILE__%(filepath)s";
const JS_RUNTIMES: [&str; 4] = ["deno", "node", "bun", "qjs"];

pub fn ytdlp_bin() -> String {
    std::env::var("DOWNLINK_YTDLP").unwrap_or_else(|_| "yt-dlp".to_string())
}

/// Primer runtime JS soportado por yt-dlp disponible en el sistema.
/// Es necesario para resolver los desafíos `n` de YouTube.
pub fn detect_js_runtime() -> Option<String> {
    if let Ok(runtime) = std::env::var("DOWNLINK_JS_RUNTIME") {
        return Some(runtime);
    }
    JS_RUNTIMES
        .into_iter()
        .find(|runtime| which(runtime))
        .map(String::from)
}

fn which(bin: &str) -> bool {
    // En Windows los ejecutables llevan extensión; se agrega más adelante si hace falta.
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(bin);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

fn is_youtube(url: &str) -> bool {
    let url = url.to_lowercase();
    url.contains("youtube.com") || url.contains("youtu.be")
}

/// Argumentos comunes a todas las invocaciones de yt-dlp.
fn common_args(settings: &Settings, url: &str) -> Vec<String> {
    let mut args = vec!["--ignore-config".to_string()];

    match settings.cookies_mode {
        CookiesMode::Browser => {
            if let Some(browser) = settings.cookies_browser.as_deref() {
                args.push("--cookies-from-browser".into());
                args.push(browser.to_string());
            }
        }
        CookiesMode::File => {
            if let Some(file) = settings.cookies_file.as_deref() {
                args.push("--cookies".into());
                args.push(file.to_string());
            }
        }
        CookiesMode::None => {}
    }

    if let Some(runtime) = detect_js_runtime() {
        args.push("--js-runtimes".into());
        args.push(runtime);
    }

    // YouTube exige resolver desafíos JS con el script oficial (EJS).
    if is_youtube(url) {
        args.push("--remote-components".into());
        args.push("ejs:github".into());
    }

    args
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
    pub formats_count: usize,
}

/// Traduce errores de yt-dlp a mensajes accionables para el usuario.
fn friendly_error(stderr: &str) -> String {
    let raw = last_error_line(stderr).unwrap_or_else(|| "La operación falló.".to_string());
    let lower = raw.to_lowercase();

    if lower.contains("sign in to confirm") || lower.contains("page needs to be reloaded") {
        return format!(
            "YouTube pidió verificación anti-bot. Abre Ajustes y activa «cookies desde el navegador» (con sesión iniciada en YouTube).\n\nDetalle: {raw}"
        );
    }
    if lower.contains("n challenge") || lower.contains("ejs") {
        return format!(
            "No se pudieron resolver los desafíos JS de YouTube. Instala Node.js o Deno, o revisa tu conexión.\n\nDetalle: {raw}"
        );
    }
    if lower.contains("could not copy") || lower.contains("failed to decrypt") {
        return format!(
            "No se pudieron leer las cookies del navegador. Cierra el navegador e intenta de nuevo, o usa un archivo cookies.txt.\n\nDetalle: {raw}"
        );
    }
    raw
}

/// Consulta los metadatos de un enlace sin descargar nada.
pub async fn probe(settings: &Settings, url: &str) -> Result<ProbeInfo, String> {
    let mut args = common_args(settings, url);
    args.extend([
        "--skip-download".into(),
        "--no-playlist".into(),
        "--no-warnings".into(),
        "--dump-single-json".into(),
        url.to_string(),
    ]);

    let output = tokio::time::timeout(
        Duration::from_secs(120),
        Command::new(ytdlp_bin())
            .args(&args)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "El sitio no respondió a tiempo. Intenta de nuevo.".to_string())?
    .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(friendly_error(&String::from_utf8_lossy(&output.stderr)));
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
        formats_count: json
            .get("formats")
            .and_then(|v| v.as_array())
            .map(|f| f.len())
            .unwrap_or(0),
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

pub type Jobs = Arc<Mutex<std::collections::HashMap<u64, Child>>>;

/// Lanza la descarga en segundo plano y emite eventos `download-progress`.
pub async fn spawn_download(
    app: AppHandle,
    job_id: u64,
    url: String,
    out_dir: PathBuf,
    jobs: Jobs,
    settings: Settings,
) -> Result<(), String> {
    let mut args = common_args(&settings, &url);
    args.extend([
        "--no-playlist".into(),
        "--no-warnings".into(),
        "--newline".into(),
        "--progress".into(),
        "--progress-template".into(),
        PROGRESS_TEMPLATE.into(),
        "--print".into(),
        FILE_TEMPLATE.into(),
        "--output".into(),
        "%(title)s [%(id)s].%(ext)s".into(),
        url.clone(),
    ]);

    let mut child = Command::new(ytdlp_bin())
        .args(&args)
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
                message: Some(friendly_error(&stderr_text)),
            }
        };
        let _ = app.emit(DOWNLOAD_EVENT, payload);
    });

    Ok(())
}

/// Detiene una descarga en curso. Devuelve `true` si había algo que cancelar.
pub async fn cancel(jobs: &Jobs, job_id: u64) -> bool {
    let mut map = jobs.lock().await;
    if let Some(mut child) = map.remove(&job_id) {
        let _ = child.kill().await;
        true
    } else {
        false
    }
}

/// Información del motor para mostrarla en Ajustes.
pub async fn engine_info() -> EngineInfo {
    let version = Command::new(ytdlp_bin())
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .await
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());

    EngineInfo {
        ytdlp_version: version,
        js_runtime: detect_js_runtime(),
    }
}

fn last_error_line(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(mode: CookiesMode, browser: Option<&str>, file: Option<&str>) -> Settings {
        Settings {
            cookies_mode: mode,
            cookies_browser: browser.map(String::from),
            cookies_file: file.map(String::from),
        }
    }

    #[test]
    fn incluye_cookies_del_navegador() {
        let settings = settings_with(CookiesMode::Browser, Some("brave"), None);
        let args = common_args(&settings, "https://example.com/video");
        assert!(
            args.windows(2)
                .any(|w| w == ["--cookies-from-browser", "brave"])
        );
    }

    #[test]
    fn incluye_archivo_de_cookies() {
        let settings = settings_with(CookiesMode::File, None, Some("/tmp/cookies.txt"));
        let args = common_args(&settings, "https://example.com/video");
        assert!(
            args.windows(2)
                .any(|w| w == ["--cookies", "/tmp/cookies.txt"])
        );
    }

    #[test]
    fn sin_cookies_no_agrega_flags() {
        let settings = settings_with(CookiesMode::None, Some("brave"), None);
        let args = common_args(&settings, "https://example.com/video");
        assert!(!args.iter().any(|a| a == "--cookies-from-browser"));
        assert!(!args.iter().any(|a| a == "--cookies"));
    }

    #[test]
    fn agrega_ejs_solo_para_youtube() {
        let settings = Settings::default();
        let youtube = common_args(&settings, "https://youtu.be/abc");
        let otro = common_args(&settings, "https://vimeo.com/123");
        assert!(youtube.iter().any(|a| a == "ejs:github"));
        assert!(!otro.iter().any(|a| a == "ejs:github"));
    }

    #[test]
    fn detecta_dominios_de_youtube() {
        assert!(is_youtube("https://www.youtube.com/watch?v=x"));
        assert!(is_youtube("https://YOUTU.BE/x"));
        assert!(!is_youtube("https://vimeo.com/1"));
    }

    #[test]
    fn errores_anti_bot_sugieren_ajustes() {
        let msg = friendly_error(
            "WARNING: algo\nERROR: [youtube] x: Sign in to confirm you're not a bot. Use --cookies-from-browser",
        );
        assert!(msg.contains("Ajustes"));
        assert!(msg.contains("Sign in to confirm"));
    }

    #[test]
    fn lee_numeros_con_valores_na() {
        assert_eq!(opt_f64("NA"), None);
        assert_eq!(opt_f64(""), None);
        assert_eq!(opt_f64("12.5"), Some(12.5));
        assert_eq!(opt_f64("100"), Some(100.0));
    }
}
