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

fn default_theme_id() -> String {
    "downlink-dark".to_string()
}

fn default_wallpaper_opacity() -> f64 {
    0.35
}

fn default_wallpaper_blur() -> f64 {
    12.0
}

/// Apariencia: tema, acento e imagen de fondo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSettings {
    /// Id de un tema del catálogo, o `"system"` para seguir al escritorio.
    #[serde(default = "default_theme_id")]
    pub theme_id: String,
    /// Acento personalizado en hex (`#rrggbb`), o `None` para el del tema.
    #[serde(default)]
    pub accent: Option<String>,
    /// Ruta a la imagen de fondo elegida por el usuario.
    #[serde(default)]
    pub wallpaper: Option<String>,
    /// Cuánto se ve la imagen de fondo (0–1).
    #[serde(default = "default_wallpaper_opacity")]
    pub wallpaper_opacity: f64,
    /// Desenfoque de la imagen de fondo, en píxeles.
    #[serde(default = "default_wallpaper_blur")]
    pub wallpaper_blur: f64,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            theme_id: default_theme_id(),
            accent: None,
            wallpaper: None,
            wallpaper_opacity: default_wallpaper_opacity(),
            wallpaper_blur: default_wallpaper_blur(),
        }
    }
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
    /// Carpeta de destino de las descargas. `None` = la estándar del sistema.
    #[serde(default)]
    pub download_dir: Option<String>,
    /// Reanuda las descargas pendientes al abrir la app.
    #[serde(default = "default_true")]
    pub persist_queue: bool,
    /// Avisa con una notificación del sistema al terminar cada descarga.
    #[serde(default = "default_true")]
    pub notify_on_finish: bool,
    /// Mantiene la app en marcha (en la bandeja) al cerrar la ventana.
    #[serde(default)]
    pub close_to_tray: bool,
    /// Preferencias de apariencia.
    #[serde(default)]
    pub theme: ThemeSettings,
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
            download_dir: None,
            persist_queue: true,
            notify_on_finish: true,
            close_to_tray: false,
            theme: ThemeSettings::default(),
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

/// Carpeta elegida por el usuario, si la hay (ignora cadenas vacías).
fn configured_dir(settings: &Settings) -> Option<PathBuf> {
    settings
        .download_dir
        .as_deref()
        .map(str::trim)
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

/// Carpeta donde se guardan las descargas: la elegida por el usuario o la
/// estándar del sistema. La elegida se crea si todavía no existe.
pub fn download_dir(app: &AppHandle, settings: &Settings) -> Result<PathBuf, String> {
    if let Some(dir) = configured_dir(settings) {
        if !dir.is_dir() {
            std::fs::create_dir_all(&dir).map_err(|e| {
                format!(
                    "No se pudo usar la carpeta de descargas «{}»: {e}",
                    dir.display()
                )
            })?;
        }
        return Ok(dir);
    }

    let dir = app
        .path()
        .download_dir()
        .map_err(|_| "No se encontró la carpeta de Descargas.".to_string())?;
    if !dir.is_dir() {
        return Err(format!(
            "La carpeta de Descargas «{}» no existe.",
            dir.display()
        ));
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usa_la_carpeta_configurada_si_hay() {
        let settings = Settings {
            download_dir: Some("/tmp/downlink-descargas".to_string()),
            ..Settings::default()
        };
        assert_eq!(
            configured_dir(&settings),
            Some(PathBuf::from("/tmp/downlink-descargas"))
        );
    }

    #[test]
    fn ignora_una_carpeta_vacia_o_en_blanco() {
        for value in [Some(String::new()), Some("   ".to_string()), None] {
            let settings = Settings {
                download_dir: value,
                ..Settings::default()
            };
            assert!(configured_dir(&settings).is_none());
        }
    }

    #[test]
    fn los_ajustes_antiguos_siguen_cargando() {
        // Un settings.json sin los campos nuevos usa los valores por defecto.
        let json = r#"{"cookiesMode":"none","cookiesBrowser":null,"cookiesFile":null}"#;
        let settings: Settings = serde_json::from_str(json).expect("debería deserializar");
        assert_eq!(settings.download_dir, None);
        assert!(settings.persist_queue);
        assert!(settings.notify_on_finish);
        assert!(!settings.close_to_tray);
    }
}
