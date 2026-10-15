use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use crate::settings::{CookiesMode, EngineInfo, Settings};

pub const DOWNLOAD_EVENT: &str = "download-progress";

const PROGRESS_TEMPLATE: &str = "download:__DL__%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.speed)s|%(progress.eta)s|%(progress.percent)s";
const FILE_TEMPLATE: &str = "after_move:__FILE__%(filepath)s";
const JS_RUNTIMES: [&str; 4] = ["deno", "node", "bun", "qjs"];
const NORMALIZE_FILTER: &str = "loudnorm=I=-16:TP=-1.5:LRA=11";

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
        .find(|runtime| which(runtime).is_some())
        .map(String::from)
}

/// Ubica un ejecutable en el PATH.
fn which(bin: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(bin))
            .find(|candidate| candidate.is_file())
    })
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

// ---------------------------------------------------------------------------
// Opciones de descarga
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MediaMode {
    #[default]
    Video,
    Audio,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    #[serde(default)]
    pub mode: MediaMode,
    /// Altura máxima en píxeles (720, 1080, 2160…). `None` = la mejor disponible.
    pub video_height: Option<u32>,
    /// Contenedor de video: mp4, mkv o webm.
    pub container: Option<String>,
    /// Formato de audio: mp3, m4a, opus, vorbis, flac, wav o "best" (sin recomprimir).
    pub audio_format: Option<String>,
    /// Calidad de audio de conversión: 320K, 256K, 192K, 128K.
    pub audio_quality: Option<String>,
    /// Ajuste de volumen en decibelios (-20 a +20). `None` = sin cambios.
    pub volume_db: Option<f64>,
    /// Normaliza el volumen a -16 LUFS (filtro loudnorm).
    #[serde(default)]
    pub normalize_audio: bool,
    #[serde(default)]
    pub embed_metadata: bool,
    #[serde(default)]
    pub embed_thumbnail: bool,
}

impl DownloadOptions {
    /// Filtros de ffmpeg (`-af`) derivados de las opciones.
    fn audio_filters(&self) -> String {
        let mut parts = Vec::new();
        if let Some(db) = self.volume_db
            && db.abs() > f64::EPSILON
        {
            parts.push(format!("volume={db}dB"));
        }
        if self.normalize_audio {
            parts.push(NORMALIZE_FILTER.to_string());
        }
        parts.join(",")
    }

    /// ¿Los filtros de audio obligan a re-codificar el stream?
    fn needs_audio_reencode(&self) -> bool {
        !self.audio_filters().is_empty()
    }
}

/// Cadena de selección de formato (`-f`) para video.
fn video_format(height: Option<u32>, container: Option<&str>) -> String {
    let limit = height.map(|h| format!("[height<={h}]")).unwrap_or_default();
    if container == Some("webm") {
        format!("bv*[ext=webm]{limit}+ba[ext=webm]/bv*{limit}+ba/b{limit}")
    } else {
        format!("bv*{limit}+ba/b{limit}")
    }
}

/// Argumentos completos de descarga (comunes + opciones del usuario).
pub fn build_download_args(
    settings: &Settings,
    url: &str,
    options: &DownloadOptions,
) -> Vec<String> {
    let mut args = common_args(settings, url);
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
    ]);

    let filters = options.audio_filters();

    match options.mode {
        MediaMode::Video => {
            let container = options.container.as_deref();
            args.push("-f".into());
            args.push(video_format(options.video_height, container));

            if container == Some("mp4") {
                // Compatibilidad con reproductores: preferir H.264 + AAC en MP4.
                args.push("-S".into());
                args.push("vcodec:h264,acodec:aac".into());
            }

            if options.needs_audio_reencode() {
                // Los filtros de audio requieren re-codificar: el merge usa copia de streams.
                args.push("--recode-video".into());
                args.push(container.unwrap_or("mp4").to_string());
                args.push("--postprocessor-args".into());
                args.push(format!("VideoConvertor+ffmpeg:-af {filters}"));
            } else if let Some(container) = container {
                args.push("--merge-output-format".into());
                args.push(container.to_string());
            }
        }
        MediaMode::Audio => {
            args.push("-x".into());

            // Con filtros de audio hay que convertir, no se puede copiar el stream.
            let requested = options.audio_format.as_deref().unwrap_or("best");
            let format = if requested == "best" && !filters.is_empty() {
                "m4a"
            } else {
                requested
            };

            if format != "best" {
                args.push("--audio-format".into());
                args.push(format.to_string());
                if let Some(quality) = options.audio_quality.as_deref() {
                    args.push("--audio-quality".into());
                    args.push(quality.to_string());
                }
            }

            if !filters.is_empty() {
                args.push("--postprocessor-args".into());
                args.push(format!("ExtractAudio+ffmpeg:-af {filters}"));
            }
        }
    }

    if options.embed_metadata {
        args.push("--embed-metadata".into());
    }
    if options.embed_thumbnail {
        args.push("--embed-thumbnail".into());
    }

    args.push(url.to_string());
    args
}

// ---------------------------------------------------------------------------
// Sondeo de metadatos
// ---------------------------------------------------------------------------

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
    /// Resoluciones de video disponibles, de mayor a menor.
    pub heights: Vec<u32>,
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
        heights: available_heights(&json),
    })
}

/// Resoluciones de video presentes en la respuesta de yt-dlp.
fn available_heights(json: &serde_json::Value) -> Vec<u32> {
    let mut heights: Vec<u32> = json
        .get("formats")
        .and_then(|v| v.as_array())
        .map(|formats| {
            formats
                .iter()
                .filter_map(|f| f.get("height").and_then(|h| h.as_u64()))
                .filter(|h| *h > 0)
                .map(|h| h as u32)
                .collect()
        })
        .unwrap_or_default();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    heights.truncate(8);
    heights
}

// ---------------------------------------------------------------------------
// Descarga
// ---------------------------------------------------------------------------

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
    options: DownloadOptions,
) -> Result<(), String> {
    let args = build_download_args(&settings, &url, &options);

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

    let ffmpeg_path = which("ffmpeg");
    let ffmpeg_version = match ffmpeg_path.as_deref() {
        Some(path) => ffmpeg_version(path).await,
        None => None,
    };

    EngineInfo {
        ytdlp_version: version,
        js_runtime: detect_js_runtime(),
        ffmpeg_path: ffmpeg_path.map(|p| p.display().to_string()),
        ffmpeg_version,
    }
}

async fn ffmpeg_version(path: &Path) -> Option<String> {
    let output = Command::new(path)
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .to_string();
    // "ffmpeg version 8.1.3 Copyright..." → "8.1.3"
    first_line.split_whitespace().nth(2).map(|v| v.to_string())
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

    fn no_cookies() -> Settings {
        Settings::default()
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

    #[test]
    fn video_1080p_mp4_con_metadatos() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            video_height: Some(1080),
            container: Some("mp4".into()),
            embed_metadata: true,
            embed_thumbnail: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(
            args.windows(2)
                .any(|w| w == ["-f", "bv*[height<=1080]+ba/b[height<=1080]"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["-S", "vcodec:h264,acodec:aac"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--merge-output-format", "mp4"])
        );
        assert!(args.iter().any(|a| a == "--embed-metadata"));
        assert!(args.iter().any(|a| a == "--embed-thumbnail"));
        assert!(!args.iter().any(|a| a == "--recode-video"));
    }

    #[test]
    fn video_webm_prefiere_webm() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            container: Some("webm".into()),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(
            args.iter()
                .any(|a| a == "bv*[ext=webm]+ba[ext=webm]/bv*+ba/b")
        );
        assert!(!args.iter().any(|a| a == "-S"));
    }

    #[test]
    fn mp3_con_volumen_y_normalizacion() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("mp3".into()),
            audio_quality: Some("320K".into()),
            volume_db: Some(6.0),
            normalize_audio: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(args.contains(&"-x".to_string()));
        assert!(args.windows(2).any(|w| w == ["--audio-format", "mp3"]));
        assert!(args.windows(2).any(|w| w == ["--audio-quality", "320K"]));
        let ppa = args
            .windows(2)
            .find(|w| w[0] == "--postprocessor-args")
            .expect("falta --postprocessor-args");
        assert!(ppa[1].starts_with("ExtractAudio+ffmpeg:-af volume=6dB,loudnorm"));
    }

    #[test]
    fn audio_original_con_volumen_fuerza_conversion() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("best".into()),
            volume_db: Some(-4.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.windows(2).any(|w| w == ["--audio-format", "m4a"]));
    }

    #[test]
    fn audio_original_sin_filtros_no_convierte() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("best".into()),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a == "--audio-format"));
        assert!(args.contains(&"-x".to_string()));
    }

    #[test]
    fn volumen_cero_no_genera_filtro() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("flac".into()),
            volume_db: Some(0.0),
            ..Default::default()
        };
        assert!(options.audio_filters().is_empty());
        assert!(!options.needs_audio_reencode());
    }

    #[test]
    fn video_con_volumen_re_codifica() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            video_height: Some(720),
            container: Some("mp4".into()),
            volume_db: Some(3.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(args.windows(2).any(|w| w == ["--recode-video", "mp4"]));
        assert!(
            args.iter()
                .any(|a| a.starts_with("VideoConvertor+ffmpeg:-af volume=3"))
        );
        assert!(!args.iter().any(|a| a == "--merge-output-format"));
    }

    #[test]
    fn la_url_va_al_final() {
        let options = DownloadOptions::default();
        let args = build_download_args(&no_cookies(), "https://vimeo.com/999", &options);
        assert_eq!(args.last().unwrap(), "https://vimeo.com/999");
    }

    #[test]
    fn resoluciones_disponibles_ordenadas_y_sin_repetir() {
        let json = serde_json::json!({
            "formats": [
                {"height": 720}, {"height": 1440}, {"height": 720},
                {"height": null}, {"height": 0}, {"height": 1080}
            ]
        });
        assert_eq!(available_heights(&json), vec![1440, 1080, 720]);
    }
}
