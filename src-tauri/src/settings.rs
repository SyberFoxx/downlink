use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

/// Origen de las cookies para contenido restringido (YouTube anti-bot, privados…).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CookiesMode {
    #[default]
    None,
    Browser,
    File,
}

/// Descargas simultáneas por defecto.
const DEFAULT_CONCURRENCY: u32 = 2;

fn default_concurrency() -> u32 {
    DEFAULT_CONCURRENCY
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub cookies_mode: CookiesMode,
    pub cookies_browser: Option<String>,
    pub cookies_file: Option<String>,
    /// Número de descargas simultáneas (1–8).
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    /// Busca y aplica actualizaciones de yt-dlp al arrancar.
    #[serde(default = "default_true")]
    pub auto_update_ytdlp: bool,
    /// Descarga ffmpeg al arrancar si no hay ninguno disponible.
    ///
    /// No se refresca solo: los builds de ffmpeg pesan ~150 MB y se regeneran a
    /// diario, así que actualizar una copia existente queda a mano en Ajustes.
    #[serde(default = "default_true")]
    pub auto_download_ffmpeg: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cookies_mode: CookiesMode::None,
            cookies_browser: None,
            cookies_file: None,
            concurrency: DEFAULT_CONCURRENCY,
            auto_update_ytdlp: true,
            auto_download_ffmpeg: true,
        }
    }
}

pub struct SettingsState(pub Mutex<Settings>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    pub id: String,
    pub label: String,
}

/// Navegadores con perfil detectado en este equipo (de donde se pueden extraer cookies).
pub fn detect_browsers() -> Vec<BrowserInfo> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let candidates: [(&str, &str, &str); 7] = [
        ("firefox", "Firefox", ".mozilla/firefox"),
        ("brave", "Brave", ".config/BraveSoftware/Brave-Browser"),
        ("chrome", "Google Chrome", ".config/google-chrome"),
        ("chromium", "Chromium", ".config/chromium"),
        ("edge", "Microsoft Edge", ".config/microsoft-edge"),
        ("vivaldi", "Vivaldi", ".config/vivaldi"),
        ("opera", "Opera", ".config/opera"),
    ];

    let Some(home) = home else {
        return Vec::new();
    };

    candidates
        .iter()
        .filter(|(_, _, relative)| home.join(relative).is_dir())
        .map(|(id, label, _)| BrowserInfo {
            id: (*id).to_string(),
            label: (*label).to_string(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub ytdlp_version: Option<String>,
    pub ytdlp_source: Option<String>,
    pub ytdlp_path: Option<String>,
    pub js_runtime: Option<String>,
    pub ffmpeg_path: Option<String>,
    pub ffmpeg_version: Option<String>,
    pub ffmpeg_source: Option<String>,
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| "No se pudo resolver la carpeta de configuración.".to_string())?;
    Ok(dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Ok(path) = settings_path(app) else {
        return Settings::default();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Settings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear la carpeta de configuración: {e}"))?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("No se pudo serializar la configuración: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("No se pudo guardar la configuración: {e}"))
}
