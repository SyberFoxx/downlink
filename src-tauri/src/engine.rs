//! Localización y gestión de los binarios externos (yt-dlp y ffmpeg).
//!
//! Orden de resolución de cada ejecutable:
//!
//! 1. **Variable de entorno** (`DOWNLINK_YTDLP` / `DOWNLINK_FFMPEG`): permite
//!    forzar una ruta concreta (útil en desarrollo y tests).
//! 2. **Copia gestionada** por la app en `<app_data>/bin`: es la que la app
//!    puede descargar y mantener actualizada sin tocar el sistema.
//! 3. **Sidecar** incluido junto al ejecutable (mecanismo `externalBin` de Tauri).
//! 4. **PATH** del sistema.
//!
//! `yt-dlp` nunca puede quedar sin resolver: si no se encuentra, se devuelve el
//! nombre pelado para que falle con un mensaje claro al ejecutarlo.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

/// Carpeta de binarios gestionados por la app. Se fija una sola vez al arrancar.
static MANAGED_BIN_DIR: OnceLock<PathBuf> = OnceLock::new();

/// De dónde procede un ejecutable (para mostrarlo en Ajustes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Ruta forzada por variable de entorno.
    Env,
    /// Copia propia de la app en `<app_data>/bin`.
    Managed,
    /// Binario incluido junto a la aplicación.
    Sidecar,
    /// Ejecutable del sistema (PATH).
    System,
}

impl Source {
    /// Etiqueta legible para la interfaz.
    pub fn label(self) -> &'static str {
        match self {
            Source::Env => "variable de entorno",
            Source::Managed => "gestionado por Downlink",
            Source::Sidecar => "incluido con la app",
            Source::System => "sistema",
        }
    }
}

/// Fija la carpeta de binarios gestionados a partir del directorio de datos de
/// la app. Idempotente: llamarlo más de una vez no tiene efecto.
pub fn init(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = MANAGED_BIN_DIR.set(dir.join("bin"));
    }
}

/// Carpeta de binarios gestionados, si ya se inicializó.
pub fn managed_bin_dir() -> Option<&'static Path> {
    MANAGED_BIN_DIR.get().map(PathBuf::as_path)
}

/// Nombre del ejecutable con la extensión propia de la plataforma.
fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Ruta que ocuparía la copia gestionada (exista o no todavía).
pub fn managed_path(name: &str) -> Option<PathBuf> {
    managed_bin_dir().map(|dir| dir.join(exe_name(name)))
}

/// Binario incluido junto al ejecutable actual (sidecar de Tauri).
fn sidecar_path(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let candidate = exe.parent()?.join(exe_name(name));
    candidate.is_file().then_some(candidate)
}

/// Ubica un ejecutable en el `PATH` del sistema.
pub fn which(bin: &str) -> Option<PathBuf> {
    let name = exe_name(bin);
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(&name))
            .find(|candidate| candidate.is_file())
    })
}

/// Resuelve un ejecutable y su origen siguiendo el orden documentado arriba.
pub fn resolve(name: &str, env_var: &str) -> Option<(PathBuf, Source)> {
    if let Ok(explicit) = std::env::var(env_var)
        && !explicit.trim().is_empty()
    {
        return Some((PathBuf::from(explicit), Source::Env));
    }
    if let Some(path) = managed_path(name)
        && path.is_file()
    {
        return Some((path, Source::Managed));
    }
    if let Some(path) = sidecar_path(name) {
        return Some((path, Source::Sidecar));
    }
    which(name).map(|path| (path, Source::System))
}

/// Resuelve `yt-dlp` con su origen. Nunca es `None`: si no aparece, se devuelve
/// el nombre pelado para que el error al ejecutarlo sea explicativo.
pub fn ytdlp() -> (PathBuf, Source) {
    resolve("yt-dlp", "DOWNLINK_YTDLP").unwrap_or((PathBuf::from("yt-dlp"), Source::System))
}

/// Resuelve `ffmpeg` con su origen (puede no estar instalado).
pub fn ffmpeg() -> Option<(PathBuf, Source)> {
    resolve("ffmpeg", "DOWNLINK_FFMPEG")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nombre_lleva_extension_solo_en_windows() {
        let name = exe_name("yt-dlp");
        if cfg!(windows) {
            assert_eq!(name, "yt-dlp.exe");
        } else {
            assert_eq!(name, "yt-dlp");
        }
    }

    #[test]
    fn las_etiquetas_de_origen_son_distintas() {
        assert_ne!(Source::System.label(), Source::Managed.label());
        assert_ne!(Source::Sidecar.label(), Source::Env.label());
        assert!(!Source::Managed.label().is_empty());
    }

    #[test]
    fn sin_inicializar_no_hay_carpeta_gestionada() {
        // En los tests nunca se llama a `init`, así que no hay carpeta gestionada
        // y la resolución cae a la variable de entorno / PATH.
        assert!(managed_bin_dir().is_none());
    }

    /// `yt-dlp` siempre resuelve a algo (ruta del PATH o el nombre pelado).
    #[test]
    fn ytdlp_siempre_resuelve_algo() {
        let (path, _source) = ytdlp();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        assert!(
            name == "yt-dlp" || name == "yt-dlp.exe",
            "nombre inesperado: {name}"
        );
    }
}
