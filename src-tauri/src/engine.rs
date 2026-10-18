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

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};
use tokio::process::Command;

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

/// Crea un `Command` para un binario externo (yt-dlp, ffmpeg, node…) con el
/// entorno de la AppImage saneado ([`sanitize_env`]) y sin ventana de consola en
/// Windows ([`hide_console`]).
///
/// Es el **único** punto por el que deben lanzarse procesos externos.
pub fn command(bin: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(bin);
    sanitize_env(&mut cmd);
    hide_console(&mut cmd);
    cmd
}

/// Evita que Windows abra una ventana de consola al lanzar un ejecutable de
/// consola (`yt-dlp.exe`, `ffmpeg.exe`, `qjs.exe`…) desde la app, que es
/// gráfica.
///
/// Sin la bandera `CREATE_NO_WINDOW` cada invocación hace parpadear (o deja
/// abierta) una ventana de CMD, lo que resulta alarmante para el usuario. Como
/// todos los procesos pasan por [`command`], aquí queda cubierto todo.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    /// No crear consola para el proceso hijo.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// Quita del entorno del proceso hijo las bibliotecas que inyecta la AppImage.
///
/// La AppImage exporta `LD_LIBRARY_PATH=$APPDIR/usr/lib:…` y empaqueta sus
/// propias `libcrypto.so.3`, `libssh.so.4`, `libcurl-gnutls.so.4`… Los binarios
/// del **sistema** (`ffmpeg`, `node`) heredan esa variable y cargan esos `.so`
/// en vez de los suyos, de modo que **no arrancan**:
///
/// ```text
/// /usr/bin/ffmpeg: …/usr/lib/libssh.so.4: version `LIBSSH_4_10_0' not found
/// node: …/usr/lib/libcrypto.so.3: version `OPENSSL_3.4.0' not found
/// ```
///
/// Las consecuencias visibles eran: «Postprocessing: ffmpeg not found» al
/// descargar y, sin `node`, yt-dlp no podía resolver los desafíos JS de YouTube
/// (error que apunta a las cookies). Fuera de una AppImage (`APPDIR` sin
/// definir) esta función no hace nada.
pub fn sanitize_env(cmd: &mut Command) {
    let Some(appdir) = std::env::var_os("APPDIR").map(PathBuf::from) else {
        return;
    };
    for var in ["LD_LIBRARY_PATH", "LD_PRELOAD"] {
        let Some(value) = std::env::var_os(var) else {
            continue;
        };
        match strip_paths_inside(&value, &appdir) {
            Some(clean) => {
                cmd.env(var, clean);
            }
            None => {
                cmd.env_remove(var);
            }
        }
    }
}

/// Devuelve `value` sin las entradas de ruta que caen dentro de `dir`.
/// `None` si no queda ninguna (hay que quitar la variable entera).
fn strip_paths_inside(value: &OsStr, dir: &Path) -> Option<OsString> {
    let kept: Vec<PathBuf> = std::env::split_paths(value)
        .filter(|entry| !entry.starts_with(dir))
        .collect();
    if kept.is_empty() {
        return None;
    }
    std::env::join_paths(kept).ok()
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

/// Error al no poder lanzar el motor.
///
/// En Android los motores van dentro del APK y todavía no hay ninguno (ver
/// `docs/android-v1.md`, fase C), así que conviene decir eso en vez de soltar un
/// error del sistema, que parecería una avería.
pub fn spawn_error(error: &std::io::Error) -> String {
    if cfg!(target_os = "android") {
        "Esta versión para Android todavía no incluye el motor de descarga.".to_string()
    } else {
        format!("No se pudo ejecutar yt-dlp: {error}")
    }
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
    fn el_error_de_lanzamiento_explica_que_es_lo_que_falta() {
        let error = std::io::Error::from(std::io::ErrorKind::NotFound);
        let message = spawn_error(&error);
        if cfg!(target_os = "android") {
            assert!(message.contains("Android"), "{message}");
        } else {
            assert!(message.contains("yt-dlp"), "{message}");
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

    #[test]
    #[cfg(unix)]
    fn quita_las_rutas_de_la_appimage_y_conserva_el_resto() {
        let appdir = Path::new("/tmp/.mount_Downli123");
        let value = OsStr::new("/tmp/.mount_Downli123/usr/lib:/opt/mis-libs:/usr/lib64");
        let clean = strip_paths_inside(value, appdir).expect("queda alguna ruta");
        assert_eq!(clean, OsStr::new("/opt/mis-libs:/usr/lib64"));
    }

    #[test]
    #[cfg(unix)]
    fn sin_rutas_propias_la_variable_desaparece() {
        let appdir = Path::new("/tmp/.mount_Downli123");
        let value = OsStr::new("/tmp/.mount_Downli123/usr/lib:/tmp/.mount_Downli123/lib64");
        assert_eq!(strip_paths_inside(value, appdir), None);
    }

    /// Vale en cualquier plataforma: una ruta suelta que no cae dentro de la
    /// AppImage se conserva tal cual.
    #[test]
    fn conserva_una_ruta_ajena_a_la_appimage() {
        let value = OsStr::new("/usr/lib");
        assert_eq!(
            strip_paths_inside(value, Path::new("/tmp/.mount_Downli123")),
            Some(value.to_os_string())
        );
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
