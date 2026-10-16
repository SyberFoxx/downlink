//! Descarga y actualización de los binarios gestionados por la app.
//!
//! De momento cubre **yt-dlp**: se descarga desde sus *releases* oficiales de
//! GitHub a la carpeta gestionada (`<app_data>/bin`), de donde `engine::resolve`
//! lo prefiere por delante del sidecar y del `PATH`. Antes de sustituir el
//! binario se verifica en dos pasos:
//!
//! 1. **SHA-256** contra el archivo `SHA2-256SUMS` del propio release.
//! 2. **Ejecución real** del binario descargado (`--version`), que confirma que
//!    arranca en esta plataforma.
//!
//! Solo entonces se reemplaza el binario de forma atómica (`rename`).

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::engine;
use crate::settings::SettingsState;

/// Endpoint de la última release de yt-dlp.
const YTDLP_RELEASE_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
/// Base de descarga de los assets de una release.
const YTDLP_DOWNLOAD_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/download";
/// GitHub exige un `User-Agent` con la identidad de la aplicación.
const USER_AGENT: &str = "Downlink (+https://github.com/SyberFoxx/downlink)";
/// Nombre del archivo temporal de descarga (sin extensión final).
const DOWNLOAD_STEM: &str = "yt-dlp.download";
/// Tiempo máximo para las peticiones cortas (consultas, checksums).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Canal de eventos con el progreso de la actualización del motor.
pub const ENGINE_UPDATE_EVENT: &str = "engine-update";

/// Evita dos actualizaciones simultáneas (manual + automática).
static UPDATING: AtomicBool = AtomicBool::new(false);

/// Fase del proceso de actualización (para la UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdatePhase {
    Checking,
    Downloading,
    Verifying,
    Installing,
    Done,
    Error,
}

/// Progreso enviado al frontend por el evento `engine-update`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub engine: String,
    pub phase: UpdatePhase,
    /// Porcentaje de descarga (0–100) cuando se conoce.
    pub percent: Option<f64>,
    pub message: Option<String>,
}

/// Resultado de comprobar si hay una versión nueva.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    /// Versión instalada actualmente (o `None` si no se encontró yt-dlp).
    pub current: Option<String>,
    /// Última versión publicada.
    pub latest: Option<String>,
    pub update_available: bool,
}

/// Libera el candado de actualización pase lo que pase.
struct UpdateGuard;

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        UPDATING.store(false, Ordering::SeqCst);
    }
}

fn begin_update() -> Result<UpdateGuard, String> {
    if UPDATING.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una actualización de yt-dlp en curso.".to_string());
    }
    Ok(UpdateGuard)
}

fn emit_phase(app: &AppHandle, phase: UpdatePhase, percent: Option<f64>, message: Option<String>) {
    let _ = app.emit(
        ENGINE_UPDATE_EVENT,
        UpdateProgress {
            engine: "yt-dlp".to_string(),
            phase,
            percent,
            message,
        },
    );
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("No se pudo preparar la conexión HTTP: {e}"))
}

// ---------------------------------------------------------------------------
// Selección de asset y comparación de versiones
// ---------------------------------------------------------------------------

/// Nombre del asset de yt-dlp para la plataforma actual, si existe binario oficial.
fn asset_for_platform() -> Option<&'static str> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Nombre del asset de yt-dlp para un par sistema/arquitectura (función pura).
///
/// Solo cubre los assets publicados como binario suelto. `yt-dlp_linux_armv7l`
/// únicamente se publica dentro de un `.zip`, así que no se ofrece (la app no
/// desempaqueta archivos).
fn asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("yt-dlp.exe"),
        ("windows", "x86") => Some("yt-dlp_x86.exe"),
        ("windows", "aarch64") => Some("yt-dlp_arm64.exe"),
        ("linux", "x86_64") => Some("yt-dlp_linux"),
        ("linux", "aarch64") => Some("yt-dlp_linux_aarch64"),
        ("macos", "x86_64") | ("macos", "aarch64") => Some("yt-dlp_macos"),
        _ => None,
    }
}

/// URL de descarga de un asset dentro de una release.
fn asset_url(version: &str, asset: &str) -> String {
    format!("{YTDLP_DOWNLOAD_BASE}/{version}/{asset}")
}

/// Extrae `tag_name` de la respuesta JSON de la API de GitHub.
fn parse_latest_tag(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = value.get("tag_name")?.as_str()?.trim();
    (!tag.is_empty()).then(|| tag.to_string())
}

/// ¿`latest` es más reciente que `current`? Compara componentes numéricos
/// (yt-dlp usa versiones tipo `2025.01.15` o `2025.01.15.123456`).
fn is_newer(current: &str, latest: &str) -> bool {
    fn parts(value: &str) -> Vec<u64> {
        value
            .trim()
            .split('.')
            .map(|p| p.trim().parse::<u64>().unwrap_or(0))
            .collect()
    }

    let (current, latest) = (parts(current), parts(latest));
    for index in 0..current.len().max(latest.len()) {
        let old = current.get(index).copied().unwrap_or(0);
        let new = latest.get(index).copied().unwrap_or(0);
        if old != new {
            return new > old;
        }
    }
    false
}

/// Busca el hash de un asset en el archivo `SHA2-256SUMS`.
fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        let name = fields.next()?.trim_start_matches('*');
        (name == asset).then(|| hash.to_lowercase())
    })
}

/// Ruta temporal de la descarga. En Windows conserva la extensión `.exe` para
/// poder ejecutarla al verificarla.
fn download_temp_path(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        dir.join(format!("{DOWNLOAD_STEM}.exe"))
    } else {
        dir.join(DOWNLOAD_STEM)
    }
}

// ---------------------------------------------------------------------------
// Red
// ---------------------------------------------------------------------------

/// Consulta la última versión publicada de yt-dlp.
async fn latest_version(client: &Client) -> Result<String, String> {
    let response = client
        .get(YTDLP_RELEASE_API)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("No se pudo consultar la última versión de yt-dlp: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub respondió {} al consultar la versión de yt-dlp.",
            response.status()
        ));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Respuesta ilegible de GitHub: {e}"))?;
    parse_latest_tag(&text)
        .ok_or_else(|| "GitHub no devolvió una versión válida de yt-dlp.".to_string())
}

/// Comprueba la versión instalada frente a la última publicada.
pub async fn check() -> Result<UpdateCheck, String> {
    let client = client()?;
    let latest = latest_version(&client).await?;
    let current = crate::ytdlp::ytdlp_version().await;
    let update_available = match current.as_deref() {
        Some(version) => is_newer(version, &latest),
        // Sin yt-dlp instalado, la «actualización» equivale a descargarlo.
        None => true,
    };

    Ok(UpdateCheck {
        current,
        latest: Some(latest),
        update_available,
    })
}

/// Descarga el hash esperado del asset desde el `SHA2-256SUMS` de la release.
/// Devuelve `None` si no se puede leer (la verificación por ejecución queda igual).
async fn expected_checksum(client: &Client, version: &str, asset: &str) -> Option<String> {
    let url = asset_url(version, "SHA2-256SUMS");
    let text = client
        .get(&url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .ok()?
        .text()
        .await
        .ok()?;
    checksum_for(&text, asset)
}

/// Descarga el binario a `temp`, verifica su integridad y que se ejecute.
/// Devuelve la versión que reporta el propio binario.
async fn download_and_verify(
    app: &AppHandle,
    client: &Client,
    version: &str,
    asset: &str,
    temp: &Path,
) -> Result<String, String> {
    let url = asset_url(version, asset);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("No se pudo descargar yt-dlp: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "La descarga de yt-dlp falló: GitHub respondió {}.",
            response.status()
        ));
    }

    let total = response.content_length();
    emit_phase(app, UpdatePhase::Downloading, Some(0.0), None);

    let mut response = response;
    let mut file = tokio::fs::File::create(temp)
        .await
        .map_err(|e| format!("No se pudo crear el archivo temporal: {e}"))?;
    let mut hasher = Sha256::new();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Se interrumpió la descarga: {e}"))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("No se pudo escribir el binario: {e}"))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        let percent = total.map(|total| (downloaded as f64 / total as f64 * 100.0).min(100.0));
        emit_phase(app, UpdatePhase::Downloading, percent, None);
    }

    file.flush()
        .await
        .map_err(|e| format!("No se pudo escribir el binario: {e}"))?;
    drop(file);

    if downloaded == 0 {
        return Err("La descarga de yt-dlp quedó vacía.".to_string());
    }

    emit_phase(app, UpdatePhase::Verifying, None, None);
    let digest = format!("{:x}", hasher.finalize());
    if let Some(expected) = expected_checksum(client, version, asset).await
        && !expected.eq_ignore_ascii_case(&digest)
    {
        return Err("La verificación de integridad falló (el SHA-256 no coincide).".to_string());
    }

    make_executable(temp)?;
    probe_binary(temp)
        .await
        .ok_or_else(|| "El binario descargado no se pudo ejecutar en esta plataforma.".to_string())
}

/// Ejecuta `bin --version` y devuelve su salida, si arranca correctamente.
async fn probe_binary(bin: &Path) -> Option<String> {
    let output = Command::new(bin)
        .args(["--ignore-config", "--version"])
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!version.is_empty()).then_some(version)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("No se pudo dar permisos de ejecución al binario: {e}"))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Descarga e instala la última versión de yt-dlp.
pub async fn update_ytdlp(app: &AppHandle) -> Result<UpdateCheck, String> {
    let _guard = begin_update()?;
    let client = client()?;

    emit_phase(app, UpdatePhase::Checking, None, None);
    let latest = latest_version(&client).await.inspect_err(|e| {
        emit_phase(app, UpdatePhase::Error, None, Some(e.clone()));
    })?;

    let asset = asset_for_platform().ok_or_else(|| {
        let message = format!(
            "No hay binario oficial de yt-dlp para {} {}.",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        emit_phase(app, UpdatePhase::Error, None, Some(message.clone()));
        message
    })?;

    let dir = engine::managed_bin_dir()
        .map(Path::to_path_buf)
        .ok_or_else(|| "No se pudo resolver la carpeta de binarios gestionados.".to_string())?;
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("No se pudo crear la carpeta de binarios: {e}"))?;
    let target = engine::managed_path("yt-dlp")
        .ok_or_else(|| "No se pudo resolver la ruta del binario gestionado.".to_string())?;
    let temp = download_temp_path(&dir);

    let installed = match download_and_verify(app, &client, &latest, asset, &temp).await {
        Ok(version) => version,
        Err(err) => {
            let _ = tokio::fs::remove_file(&temp).await;
            emit_phase(app, UpdatePhase::Error, None, Some(err.clone()));
            return Err(err);
        }
    };

    // Sustitución atómica: el binario solo se reemplaza si pasó las verificaciones.
    emit_phase(app, UpdatePhase::Installing, None, None);
    if let Err(err) = tokio::fs::rename(&temp, &target).await {
        let _ = tokio::fs::remove_file(&temp).await;
        let message = format!("No se pudo instalar yt-dlp (¿está en uso?): {err}");
        emit_phase(app, UpdatePhase::Error, None, Some(message.clone()));
        return Err(message);
    }

    emit_phase(
        app,
        UpdatePhase::Done,
        None,
        Some(format!("yt-dlp actualizado a {installed}")),
    );

    Ok(UpdateCheck {
        current: Some(installed),
        latest: Some(latest),
        update_available: false,
    })
}

/// Actualiza yt-dlp al arrancar si hay versión nueva y el usuario lo permite.
/// Nunca propaga errores: una actualización fallida no debe impedir usar la app.
pub async fn auto_update(app: &AppHandle) {
    // Si el usuario forzó una ruta por variable de entorno, se respeta.
    if std::env::var_os("DOWNLINK_YTDLP").is_some() {
        return;
    }

    let enabled = {
        let state = app.state::<SettingsState>();
        let settings = state.0.lock().await;
        settings.auto_update_ytdlp
    };
    if !enabled {
        return;
    }

    match check().await {
        Ok(result) if result.update_available => {
            if let Err(err) = update_ytdlp(app).await {
                eprintln!("[downlink] auto-update de yt-dlp: {err}");
            }
        }
        Ok(_) => {}
        Err(err) => eprintln!("[downlink] no se pudo comprobar la versión de yt-dlp: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hay_asset_para_plataformas_habituales() {
        assert_eq!(asset_for("linux", "x86_64"), Some("yt-dlp_linux"));
        assert_eq!(asset_for("linux", "aarch64"), Some("yt-dlp_linux_aarch64"));
        assert_eq!(asset_for("windows", "x86_64"), Some("yt-dlp.exe"));
        assert_eq!(asset_for("macos", "aarch64"), Some("yt-dlp_macos"));
        assert_eq!(asset_for("solaris", "sparc"), None);
        // linux/arm solo se publica empaquetado en .zip: no se ofrece.
        assert_eq!(asset_for("linux", "arm"), None);
    }

    #[test]
    fn el_asset_actual_es_el_de_esta_plataforma() {
        let asset = asset_for_platform().expect("esta plataforma debe tener binario");
        assert!(asset.starts_with("yt-dlp"));
    }

    #[test]
    fn construye_la_url_del_asset() {
        assert_eq!(
            asset_url("2025.01.15", "yt-dlp_linux"),
            "https://github.com/yt-dlp/yt-dlp/releases/download/2025.01.15/yt-dlp_linux"
        );
    }

    #[test]
    fn lee_el_tag_de_la_release() {
        let json = r#"{"tag_name":"2025.01.15","name":"yt-dlp 2025.01.15"}"#;
        assert_eq!(parse_latest_tag(json).as_deref(), Some("2025.01.15"));
        assert_eq!(parse_latest_tag("{}"), None);
        assert_eq!(parse_latest_tag("no es json"), None);
    }

    #[test]
    fn compara_versiones_por_componentes() {
        assert!(is_newer("2025.01.15", "2025.02.01"));
        assert!(is_newer("2024.12.31", "2025.01.01"));
        assert!(is_newer("2025.01.15", "2025.01.15.123456"));
        assert!(!is_newer("2025.02.01", "2025.01.15"));
        assert!(!is_newer("2025.01.15", "2025.01.15"));
    }

    #[test]
    fn extrae_el_hash_del_asset() {
        let sums = "\
aaaa1111  yt-dlp_linux\n\
bbbb2222  yt-dlp_macos\n\
cccc3333 *yt-dlp.exe\n";
        assert_eq!(
            checksum_for(sums, "yt-dlp_linux").as_deref(),
            Some("aaaa1111")
        );
        assert_eq!(
            checksum_for(sums, "yt-dlp.exe").as_deref(),
            Some("cccc3333")
        );
        assert_eq!(checksum_for(sums, "yt-dlp_linux_aarch64"), None);
    }

    #[test]
    fn el_temporal_lleva_exe_solo_en_windows() {
        let temp = download_temp_path(Path::new("/tmp/bin"));
        let name = temp.file_name().unwrap().to_string_lossy();
        if cfg!(windows) {
            assert_eq!(name, "yt-dlp.download.exe");
        } else {
            assert_eq!(name, "yt-dlp.download");
        }
    }

    #[test]
    fn las_fases_serializan_en_minusculas() {
        let json = serde_json::to_string(&UpdatePhase::Downloading).unwrap();
        assert_eq!(json, "\"downloading\"");
    }
}
