//! Localización y gestión de los binarios externos (yt-dlp y ffmpeg).
//!
//! Orden de resolución de cada ejecutable:
//!
//! 1. **Variable de entorno** (`DOWNLINK_YTDLP` / `DOWNLINK_FFMPEG`): permite
//!    forzar una ruta concreta (útil en desarrollo y tests).
//! 2. **Copia gestionada** por la app en `<app_data>/bin`: es la que la app
//!    puede descargar y mantener actualizada sin tocar el sistema.
//! 3. **Motor incluido en el APK** ([`bundled`], solo Android): los intérpretes y
//!    herramientas viajan dentro de la aplicación como `lib*.so`. En el escritorio
//!    no hay nada que buscar.
//! 4. **Sidecar** incluido junto al ejecutable (mecanismo `externalBin` de Tauri).
//! 5. **PATH** del sistema.
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

/// Carpeta de datos de la app. Se fija una sola vez al arrancar.
static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Carpeta de bibliotecas nativas (`nativeLibraryDir`) que manda `MainActivity`.
/// Solo tiene sentido en Android: es donde el instalador extrae los motores.
#[cfg(target_os = "android")]
static JNI_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Nombre de los motores que viajan dentro del APK (ficheros `lib*.so` en la
/// carpeta de bibliotecas nativas).
pub const BUNDLED_PYTHON: &str = "python";
/// El stdlib de Python va comprimido y solo se descomprime en Android.
#[cfg(target_os = "android")]
pub const BUNDLED_PYTHON_STDLIB: &str = "python-stdlib";
pub const BUNDLED_QUICKJS: &str = "quickjs";
pub const BUNDLED_YTDLP: &str = "yt-dlp";

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

/// Fija las carpetas de trabajo a partir del directorio de datos de la app y
/// deja listo lo que los motores necesitan para funcionar.
///
/// Idempotente: llamarlo más de una vez no tiene efecto.
pub fn init(app: &AppHandle) {
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = APP_DATA_DIR.set(dir.clone());
        let _ = MANAGED_BIN_DIR.set(dir.join("bin"));
        // Los hijos (yt-dlp sobre todo) escriben temporales: fuera de sus
        // carpetas Android no deja.
        let _ = std::fs::create_dir_all(dir.join("tmp"));
    }
    prepare_python();
}

/// Carpeta de datos de la app, si ya se inicializó.
fn app_data_dir() -> Option<&'static Path> {
    APP_DATA_DIR.get().map(PathBuf::as_path)
}

/// Carpeta de binarios gestionados, si ya se inicializó.
pub fn managed_bin_dir() -> Option<&'static Path> {
    MANAGED_BIN_DIR.get().map(PathBuf::as_path)
}

/// `PYTHONHOME` del intérprete que viaja en el APK: el stdlib descomprimido.
pub fn python_home() -> Option<PathBuf> {
    app_data_dir().map(|dir| dir.join("python"))
}

/// Directorio de bibliotecas nativas del APK (`nativeLibraryDir`).
///
/// Es donde Android extrae los `lib*.so` y, desde Android 10, el único sitio de
/// la aplicación desde el que se puede **ejecutar** algo propio: en la carpeta de
/// datos está prohibido.
///
/// La fuente principal es `MainActivity`, que la manda por JNI
/// ([`set_bundled_lib_dir`]) usando `applicationInfo.nativeLibraryDir`; Android
/// recalcula esa ruta en cada instalación, así que preguntársela a él es lo único
/// fiable. Si eso no ha llegado todavía, se deduce con [`dladdr`] sobre esta misma
/// biblioteca, que suele bastar como respaldo.
#[cfg(target_os = "android")]
fn bundled_lib_dir() -> Option<&'static Path> {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(dir) = JNI_DIR.get() {
            return dir.clone();
        }
        detect_bundled_lib_dir().unwrap_or_default()
    });
    let dir = DIR.get().map(PathBuf::as_path)?;
    (!dir.as_os_str().is_empty()).then_some(dir)
}

/// Registra la carpeta de bibliotecas nativas que manda `MainActivity`.
///
/// Se llama una sola vez, en cuanto arranca la actividad: la primera descarga no
/// puede llegar antes.
#[cfg(target_os = "android")]
pub fn set_bundled_lib_dir(dir: &str) {
    if dir.is_empty() {
        return;
    }
    let _ = JNI_DIR.set(PathBuf::from(dir));
}

/// Deduce la carpeta de bibliotecas nativas a partir de la biblioteca ya cargada.
#[cfg(target_os = "android")]
fn detect_bundled_lib_dir() -> Option<PathBuf> {
    use std::ffi::CStr;
    use std::os::unix::ffi::OsStrExt;

    // dladdr sobre esta propia función: el nombre que devuelve es el del fichero
    // `libdownlink_lib.so`, y de ahí sale su carpeta.
    let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
    let addr = detect_bundled_lib_dir as *const libc::c_void;
    if unsafe { libc::dladdr(addr, &mut info) } != 0 && !info.dli_fname.is_null() {
        let file = unsafe { CStr::from_ptr(info.dli_fname) };
        let path = Path::new(OsStr::from_bytes(file.to_bytes()));
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            return Some(parent.to_path_buf());
        }
    }

    // Respaldo: la ruta completa aparece en `/proc/self/maps`.
    let content = std::fs::read_to_string("/proc/self/maps").ok()?;
    for line in content.lines() {
        let Some(dir) = line.rsplit_once(" /").map(|(_, rest)| rest) else {
            continue;
        };
        let Some(file) = dir.rsplit('/').next() else {
            continue;
        };
        if file.starts_with("libdownlink_lib") && file.ends_with(".so") {
            return Path::new(dir).parent().map(Path::to_path_buf);
        }
    }
    None
}

#[cfg(not(target_os = "android"))]
fn bundled_lib_dir() -> Option<&'static Path> {
    None
}

/// Ruta de un motor que viaja dentro del APK (`lib<name>.so`), si esta versión lo
/// incluye. En el escritorio siempre es `None`.
pub fn bundled(name: &str) -> Option<PathBuf> {
    let path = bundled_lib_dir()?.join(format!("lib{name}.so"));
    path.is_file().then_some(path)
}

/// Descomprime el intérprete de Python que viaja en el APK.
///
/// El stdlib no puede quedarse dentro de un `.so`: las extensiones (`_ssl.so`,
/// `_socket.so`…) tienen que ser ficheros de verdad para que Python pueda
/// cargarlas. Se hace una sola vez (la primera vez que arranca la app tras
/// instalarla o actualizarla) y se marca con una huella del paquete para no
/// repetirlo. Si falla, no se avisa aquí: el motor dará un error al usarlo.
#[cfg(target_os = "android")]
fn prepare_python() {
    use sha2::{Digest, Sha256};

    let (Some(home), Some(package)) = (python_home(), bundled(BUNDLED_PYTHON_STDLIB)) else {
        return;
    };
    let Ok(mut file) = std::fs::File::open(&package) else {
        return;
    };
    let mut hasher = Sha256::new();
    if std::io::copy(&mut file, &mut hasher).is_err() {
        return;
    }
    let fingerprint = format!("{:x}", hasher.finalize());

    let marker = home.join(".huella");
    if std::fs::read_to_string(&marker).ok().as_deref() == Some(fingerprint.as_str()) {
        return;
    }
    // Se tira lo anterior: si el APK nuevo trae otra versión de Python, los
    // módulos compilados tampoco valen.
    let lib = home.join("lib");
    let _ = std::fs::remove_dir_all(&lib);
    if extract_zip(&package, &lib).is_err() {
        return;
    }
    let _ = std::fs::write(&marker, &fingerprint);
}

#[cfg(not(target_os = "android"))]
fn prepare_python() {}

/// Descomprime un `.zip` dentro de `target`, sin dejar que una entrada se salga
/// de la carpeta ([`zip::read::ZipFile::enclosed_name`]).
#[cfg(target_os = "android")]
fn extract_zip(package: &Path, target: &Path) -> std::io::Result<()> {
    let file = std::fs::File::open(package)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        let destination = target.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut out = std::fs::File::create(&destination)?;
        std::io::copy(&mut entry, &mut out)?;
    }
    Ok(())
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
/// entorno de la AppImage saneado ([`sanitize_env`]), el de Android completado
/// ([`android_env`]) y sin ventana de consola en Windows ([`hide_console`]).
///
/// Es el **único** punto por el que deben lanzarse procesos externos.
pub fn command(bin: impl AsRef<OsStr>) -> Command {
    let mut cmd = Command::new(bin);
    sanitize_env(&mut cmd);
    android_env(&mut cmd);
    hide_console(&mut cmd);
    cmd
}

/// Completa el entorno que los motores esperan y que en Android no existe.
///
/// En Android `HOME` no está definido y el `/tmp` del sistema no es escribible,
/// y yt-dlp los necesita: guarda su caché en `HOME` (de ahí salen los componentes
/// que resuelven los retos de YouTube) y deja ficheros temporales al descargar.
/// Se le da la carpeta de datos de la app, que es donde puede escribir.
///
/// Además apunta `SSL_CERT_FILE` a los certificados raíz que viajan con el
/// intérprete. Sin ellos Python arranca pero rechaza cualquier HTTPS
/// (`unable to get local issuer certificate`) y yt-dlp no llega ni a YouTube:
/// el `openssldir` de OpenSSL no se copia al APK, así que el fichero va dentro
/// del stdlib y hay que señalarlo a mano.
#[cfg(target_os = "android")]
fn android_env(cmd: &mut Command) {
    if let Some(dir) = app_data_dir() {
        cmd.env("HOME", dir);
        cmd.env("TMPDIR", dir.join("tmp"));
    }
    if let Some(ca) = ca_bundle() {
        cmd.env("SSL_CERT_FILE", &ca);
        // `requests`/`certifi` y otras bibliotecas miran también `REQUESTS_CA_BUNDLE`.
        cmd.env("REQUESTS_CA_BUNDLE", &ca);
    }
}

/// Ruta de los certificados raíz de TLS que acompañan al intérprete del APK.
///
/// Es el `cacert.pem` que `scripts/android-engine.sh` mete en el stdlib (ver
/// [`prepare_python`], que lo descomprime en `python_home`). Se busca en las
/// versiones de `lib/python3.x` en vez de fijar la versión: si algún día se
/// sube CPython, esto sigue encontrándolo.
#[cfg(target_os = "android")]
pub fn ca_bundle() -> Option<PathBuf> {
    let lib = python_home()?.join("lib");
    for entry in std::fs::read_dir(&lib).ok()?.flatten() {
        if !entry.file_name().to_string_lossy().starts_with("python3") {
            continue;
        }
        let path = entry.path().join("cacert.pem");
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

#[cfg(not(target_os = "android"))]
fn android_env(_cmd: &mut Command) {}

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
    if let Some(path) = bundled(name) {
        return Some((path, Source::Sidecar));
    }
    if let Some(path) = sidecar_path(name) {
        return Some((path, Source::Sidecar));
    }
    which(name).map(|path| (path, Source::System))
}

/// Resuelve `yt-dlp` con su origen (puede no estar instalado).
pub fn ytdlp() -> Option<(PathBuf, Source)> {
    resolve("yt-dlp", "DOWNLINK_YTDLP")
}

/// Proceso de `yt-dlp` listo para añadirle los argumentos.
///
/// En el escritorio es su propio binario. En Android no hay binario nativo: el
/// *zipapp* lo lee el intérprete de Python que viaja en el APK, y hay que pasarle
/// dónde está el stdlib (`PYTHONHOME`). Manda la copia gestionada —la que la app
/// mantiene al día— y el zipapp del APK sirve para funcionar nada más instalar.
pub fn ytdlp_command() -> Command {
    let app = managed_path("yt-dlp")
        .filter(|path| path.is_file())
        .or_else(|| bundled(BUNDLED_YTDLP))
        .unwrap_or_else(|| PathBuf::from("yt-dlp"));
    ytdlp_command_for(&app)
}

/// Proceso de `yt-dlp` para un fichero concreto (el gestionado, el del APK o uno
/// recién descargado que todavía no está en su sitio).
pub fn ytdlp_command_for(app: &Path) -> Command {
    let Some(python) = bundled(BUNDLED_PYTHON) else {
        return command(app);
    };
    let mut cmd = command(python);
    if let Some(home) = python_home() {
        cmd.env("PYTHONHOME", home);
    }
    cmd.arg(app);
    cmd
}

/// Resuelve `ffmpeg` con su origen (puede no estar instalado).
pub fn ffmpeg() -> Option<(PathBuf, Source)> {
    resolve("ffmpeg", "DOWNLINK_FFMPEG")
}

/// Error al no poder lanzar el motor.
///
/// En Android los motores van dentro del APK, así que un fallo del sistema no
/// explica gran cosa: casi siempre será que a esta compilación le faltan los
/// binarios (`scripts/android-engine.sh`).
pub fn spawn_error(error: &std::io::Error) -> String {
    if cfg!(target_os = "android") {
        "No se pudo arrancar el motor de descarga de esta versión para Android.".to_string()
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

    /// En el escritorio, `yt-dlp` se lanza tal cual; en Android, lo lee el
    /// intérprete de Python que viaja en el APK.
    #[test]
    fn ytdlp_se_lanza_con_lo_que_haya() {
        let command = ytdlp_command();
        let program = command.as_std().get_program().to_string_lossy().to_string();
        let name = Path::new(&program)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if cfg!(target_os = "android") {
            assert_eq!(name, "libpython.so", "programa inesperado: {program}");
        } else {
            assert!(
                name == "yt-dlp" || name == "yt-dlp.exe",
                "programa inesperado: {program}"
            );
        }
    }

    /// Fuera de Android no hay motores incluidos en la aplicación.
    #[test]
    #[cfg(not(target_os = "android"))]
    fn en_el_escritorio_no_hay_motores_incluidos() {
        assert!(bundled_lib_dir().is_none());
        assert!(bundled(BUNDLED_YTDLP).is_none());
        assert!(bundled(BUNDLED_PYTHON).is_none());
    }
}
