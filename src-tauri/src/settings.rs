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
    /// Descarga un runtime JS (quickjs) al arrancar si no hay ninguno.
    ///
    /// yt-dlp lo necesita para resolver los desafíos `n` de YouTube. Es un
    /// binario diminuto (~2,5 MB), así que descargarlo evita tener que instalar
    /// Node.js a mano.
    #[serde(default = "default_true")]
    pub auto_download_js_runtime: bool,
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
            auto_download_js_runtime: true,
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
///
/// Los perfiles viven en sitios distintos según el sistema: en Windows bajo
/// `%LOCALAPPDATA%`/`%APPDATA%`, en macOS bajo `~/Library/Application Support` y
/// en Linux bajo `$XDG_CONFIG_HOME` (o `~/.config`). Comprobar solo rutas de
/// Linux dejaba la detección vacía en Windows.
pub fn detect_browsers() -> Vec<BrowserInfo> {
    browser_candidates(std::env::consts::OS, &BrowserDirs::from_env())
        .into_iter()
        .filter(|(_, _, dir)| dir.is_dir())
        .map(|(id, label, _)| BrowserInfo {
            id: id.to_string(),
            label: label.to_string(),
        })
        .collect()
}

/// Carpetas base desde las que se resuelven los perfiles de los navegadores.
#[derive(Debug, Default)]
struct BrowserDirs {
    home: Option<PathBuf>,
    local_app_data: Option<PathBuf>,
    app_data: Option<PathBuf>,
    xdg_config: Option<PathBuf>,
}

impl BrowserDirs {
    /// Lee las carpetas base del entorno. En Linux, si no hay
    /// `XDG_CONFIG_HOME`, se usa `~/.config` como manda la especificación.
    fn from_env() -> Self {
        let home = env_path("HOME").or_else(|| env_path("USERPROFILE"));
        let xdg_config =
            env_path("XDG_CONFIG_HOME").or_else(|| home.as_ref().map(|h| h.join(".config")));
        Self {
            home,
            local_app_data: env_path("LOCALAPPDATA"),
            app_data: env_path("APPDATA"),
            xdg_config,
        }
    }
}

/// Lee una variable de entorno como ruta, ignorando las vacías.
fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

/// Carpeta base de un perfil, según la plataforma.
#[derive(Debug, Clone, Copy)]
enum Base {
    Home,
    LocalAppData,
    AppData,
    XdgConfig,
}

impl Base {
    fn resolve(self, dirs: &BrowserDirs) -> Option<PathBuf> {
        match self {
            Base::Home => dirs.home.clone(),
            Base::LocalAppData => dirs.local_app_data.clone(),
            Base::AppData => dirs.app_data.clone(),
            Base::XdgConfig => dirs.xdg_config.clone(),
        }
    }
}

/// Perfiles de navegador candidatos para una plataforma, como rutas absolutas.
/// Función pura (recibe las carpetas base) para poder probarla sin depender del
/// sistema en el que corran los tests.
fn browser_candidates(os: &str, dirs: &BrowserDirs) -> Vec<(&'static str, &'static str, PathBuf)> {
    let entries: &[(&'static str, &'static str, Base, &'static str)] = match os {
        "windows" => &[
            (
                "firefox",
                "Firefox",
                Base::AppData,
                r"Mozilla\Firefox\Profiles",
            ),
            (
                "brave",
                "Brave",
                Base::LocalAppData,
                r"BraveSoftware\Brave-Browser\User Data",
            ),
            (
                "chrome",
                "Google Chrome",
                Base::LocalAppData,
                r"Google\Chrome\User Data",
            ),
            (
                "chromium",
                "Chromium",
                Base::LocalAppData,
                r"Chromium\User Data",
            ),
            (
                "edge",
                "Microsoft Edge",
                Base::LocalAppData,
                r"Microsoft\Edge\User Data",
            ),
            (
                "vivaldi",
                "Vivaldi",
                Base::LocalAppData,
                r"Vivaldi\User Data",
            ),
            (
                "opera",
                "Opera",
                Base::AppData,
                r"Opera Software\Opera Stable",
            ),
        ],
        "macos" => &[
            (
                "firefox",
                "Firefox",
                Base::Home,
                "Library/Application Support/Firefox",
            ),
            (
                "brave",
                "Brave",
                Base::Home,
                "Library/Application Support/BraveSoftware/Brave-Browser",
            ),
            (
                "chrome",
                "Google Chrome",
                Base::Home,
                "Library/Application Support/Google/Chrome",
            ),
            (
                "chromium",
                "Chromium",
                Base::Home,
                "Library/Application Support/Chromium",
            ),
            (
                "edge",
                "Microsoft Edge",
                Base::Home,
                "Library/Application Support/Microsoft Edge",
            ),
            (
                "vivaldi",
                "Vivaldi",
                Base::Home,
                "Library/Application Support/Vivaldi",
            ),
            (
                "opera",
                "Opera",
                Base::Home,
                "Library/Application Support/com.operasoftware.Opera",
            ),
        ],
        // Linux y demás sistemas tipo Unix.
        _ => &[
            ("firefox", "Firefox", Base::Home, ".mozilla/firefox"),
            (
                "brave",
                "Brave",
                Base::XdgConfig,
                "BraveSoftware/Brave-Browser",
            ),
            ("chrome", "Google Chrome", Base::XdgConfig, "google-chrome"),
            ("chromium", "Chromium", Base::XdgConfig, "chromium"),
            ("edge", "Microsoft Edge", Base::XdgConfig, "microsoft-edge"),
            ("vivaldi", "Vivaldi", Base::XdgConfig, "vivaldi"),
            ("opera", "Opera", Base::XdgConfig, "opera"),
        ],
    };

    entries
        .iter()
        .filter_map(|(id, label, base, relative)| {
            let root = base.resolve(dirs)?;
            Some((*id, *label, join_components(&root, relative)))
        })
        .collect()
}

/// Une una subruta (con `/` o `\`) a una base con el separador nativo, para que
/// el resultado sea el mismo en cualquier plataforma.
fn join_components(root: &std::path::Path, relative: &str) -> PathBuf {
    relative
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub ytdlp_version: Option<String>,
    pub ytdlp_source: Option<String>,
    pub ytdlp_path: Option<String>,
    pub js_runtime: Option<String>,
    pub js_runtime_source: Option<String>,
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
        assert!(settings.auto_download_js_runtime);
    }

    /// Base de prueba portable (los tests corren en cualquier sistema).
    fn dirs(
        home: &str,
        local: Option<&str>,
        roaming: Option<&str>,
        xdg: Option<&str>,
    ) -> BrowserDirs {
        BrowserDirs {
            home: Some(PathBuf::from(home)),
            local_app_data: local.map(PathBuf::from),
            app_data: roaming.map(PathBuf::from),
            xdg_config: xdg.map(PathBuf::from),
        }
    }

    fn dir_of<'a>(
        candidates: &'a [(&'static str, &'static str, PathBuf)],
        id: &str,
    ) -> &'a PathBuf {
        &candidates
            .iter()
            .find(|(browser, _, _)| *browser == id)
            .expect("el navegador debería estar en la lista")
            .2
    }

    #[test]
    fn en_windows_los_perfiles_estan_bajo_appdata() {
        let dirs = dirs(
            "C:/Users/ana",
            Some("C:/Users/ana/AppData/Local"),
            Some("C:/Users/ana/AppData/Roaming"),
            None,
        );
        let candidates = browser_candidates("windows", &dirs);
        assert_eq!(candidates.len(), 7);
        assert_eq!(
            dir_of(&candidates, "brave"),
            &PathBuf::from("C:/Users/ana/AppData/Local")
                .join("BraveSoftware")
                .join("Brave-Browser")
                .join("User Data")
        );
        assert_eq!(
            dir_of(&candidates, "firefox"),
            &PathBuf::from("C:/Users/ana/AppData/Roaming")
                .join("Mozilla")
                .join("Firefox")
                .join("Profiles")
        );
    }

    #[test]
    fn en_linux_los_perfiles_estan_bajo_xdg_config() {
        let dirs = dirs("/home/ana", None, None, Some("/home/ana/.config"));
        let candidates = browser_candidates("linux", &dirs);
        assert_eq!(
            dir_of(&candidates, "brave"),
            &PathBuf::from("/home/ana/.config")
                .join("BraveSoftware")
                .join("Brave-Browser")
        );
        assert_eq!(
            dir_of(&candidates, "firefox"),
            &PathBuf::from("/home/ana").join(".mozilla").join("firefox")
        );
    }

    #[test]
    fn en_macos_los_perfiles_estan_bajo_library() {
        let dirs = dirs("/Users/ana", None, None, None);
        let candidates = browser_candidates("macos", &dirs);
        assert_eq!(
            dir_of(&candidates, "chrome"),
            &PathBuf::from("/Users/ana")
                .join("Library")
                .join("Application Support")
                .join("Google")
                .join("Chrome")
        );
    }

    #[test]
    fn sin_carpetas_base_no_hay_candidatos() {
        let candidates = browser_candidates("windows", &BrowserDirs::default());
        assert!(candidates.is_empty());
    }
}
