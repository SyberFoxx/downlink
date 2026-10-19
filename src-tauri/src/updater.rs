//! Descarga y actualización de los binarios gestionados por la app.
//!
//! Cubre dos motores:
//!
//! - **yt-dlp**: binario suelto desde sus *releases* oficiales de GitHub
//!   (`api.github.com/.../releases/latest`).
//! - **ffmpeg**: builds estáticos de [BtbN/FFmpeg-Builds] (Linux/Windows), que se
//!   publican como `.tar.xz`/`.zip` y hay que desempaquetar. Se instalan `ffmpeg`
//!   y `ffprobe` juntos en `<app_data>/bin`.
//!
//! En ambos casos el binario se verifica antes de sustituir nada:
//!
//! 1. **SHA-256** contra el archivo de sumas del release (si está disponible).
//! 2. **Ejecución real** (`--version` / `-version`) del binario descargado.
//!
//! Solo entonces se reemplaza de forma atómica (`rename`).
//!
//! [BtbN/FFmpeg-Builds]: https://github.com/BtbN/FFmpeg-Builds

use std::fs::File as StdFile;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use reqwest::Client;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::engine::{self, Source};
use crate::settings::SettingsState;

/// Endpoints y bases de descarga.
const YTDLP_RELEASE_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
const YTDLP_DOWNLOAD_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/download";
const FFMPEG_RELEASE_API: &str =
    "https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/latest";
const FFMPEG_DOWNLOAD_BASE: &str = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest";
/// Runtime JS gestionado (quickjs-ng). yt-dlp lo usa para los desafíos de YouTube.
const QUICKJS_RELEASE_API: &str = "https://api.github.com/repos/quickjs-ng/quickjs/releases/latest";
const QUICKJS_DOWNLOAD_BASE: &str = "https://github.com/quickjs-ng/quickjs/releases/download";

/// Nombre del motor tal cual viaja en el evento `engine-update`.
const ENGINE_YTDLP: &str = "yt-dlp";
const ENGINE_FFMPEG: &str = "ffmpeg";
const ENGINE_QUICKJS: &str = "quickjs";

/// Nombre del binario gestionado del runtime JS (yt-dlp lo busca como `qjs`).
const QUICKJS_BIN: &str = "qjs";

/// GitHub exige un `User-Agent` con la identidad de la aplicación.
const USER_AGENT: &str = "Downlink (+https://github.com/SyberFoxx/downlink)";
/// Tiempo máximo para las peticiones cortas (consultas y sumas de control).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Canal de eventos con el progreso de la actualización del motor.
pub const ENGINE_UPDATE_EVENT: &str = "engine-update";

/// Evita dos actualizaciones simultáneas del mismo motor.
static YTDLP_UPDATING: AtomicBool = AtomicBool::new(false);
static FFMPEG_UPDATING: AtomicBool = AtomicBool::new(false);
static QUICKJS_UPDATING: AtomicBool = AtomicBool::new(false);

/// Último progreso emitido por `engine-update`.
///
/// La actualización arranca en segundo plano al abrir la app, a menudo antes de
/// que la UI exista, y un evento que nadie escucha se pierde. Guardarlo aquí
/// permite que la interfaz se hidrate al montarse y muestre la barra aunque la
/// descarga haya empezado hace unos milisegundos. Solo vive en memoria: cada
/// arranque empieza de cero.
static LAST_PROGRESS: Mutex<Option<UpdateProgress>> = Mutex::new(None);

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
    /// Versión instalada actualmente (o `None` si no se encontró el motor).
    pub current: Option<String>,
    /// Última versión publicada.
    pub latest: Option<String>,
    pub update_available: bool,
}

/// Libera el candado de actualización pase lo que pase.
struct UpdateGuard(&'static AtomicBool);

impl Drop for UpdateGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn begin_update(flag: &'static AtomicBool) -> Result<UpdateGuard, String> {
    if flag.swap(true, Ordering::SeqCst) {
        return Err("Ya hay una actualización de este motor en curso.".to_string());
    }
    Ok(UpdateGuard(flag))
}

fn emit_phase(
    app: &AppHandle,
    engine: &str,
    phase: UpdatePhase,
    percent: Option<f64>,
    message: Option<String>,
) {
    let progress = UpdateProgress {
        engine: engine.to_string(),
        phase,
        percent,
        message,
    };
    record_progress(&progress);
    let _ = app.emit(ENGINE_UPDATE_EVENT, progress);
}

/// Recuerda el último progreso para poder consultarlo desde la UI.
fn record_progress(progress: &UpdateProgress) {
    if let Ok(mut last) = LAST_PROGRESS.lock() {
        *last = Some(progress.clone());
    }
}

/// Último progreso de motor visto en esta sesión (`None` si aún no ha habido
/// ninguno). Es lo que consulta la UI al montarse.
pub fn last_progress() -> Option<UpdateProgress> {
    LAST_PROGRESS.lock().ok().and_then(|last| last.clone())
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("No se pudo preparar la conexión HTTP: {e}"))
}

// ---------------------------------------------------------------------------
// Utilidades compartidas
// ---------------------------------------------------------------------------

/// Descarga un texto corto (consultas de versión, archivos de sumas).
async fn fetch_text(client: &Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("No se pudo conectar con el servidor: {e}"))?;

    if !response.status().is_success() {
        return Err(format!("El servidor respondió {}.", response.status()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Respuesta ilegible del servidor: {e}"))
}

/// Descarga un archivo por trozos, emitiendo el progreso, y devuelve su SHA-256.
async fn stream_download(
    app: &AppHandle,
    engine: &str,
    client: &Client,
    url: &str,
    dest: &Path,
) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("No se pudo descargar: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "La descarga falló: el servidor respondió {}.",
            response.status()
        ));
    }

    let total = response.content_length();
    emit_phase(app, engine, UpdatePhase::Downloading, Some(0.0), None);

    let mut response = response;
    let mut file = tokio::fs::File::create(dest)
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
            .map_err(|e| format!("No se pudo escribir el archivo: {e}"))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        let percent = total.map(|total| (downloaded as f64 / total as f64 * 100.0).min(100.0));
        emit_phase(app, engine, UpdatePhase::Downloading, percent, None);
    }

    file.flush()
        .await
        .map_err(|e| format!("No se pudo escribir el archivo: {e}"))?;
    drop(file);

    if downloaded == 0 {
        return Err("La descarga quedó vacía.".to_string());
    }

    Ok(format!("{:x}", hasher.finalize()))
}

/// Consulta el hash esperado de un asset en un archivo de sumas (`<hash>  <nombre>`).
async fn expected_checksum(client: &Client, sums_url: &str, asset: &str) -> Option<String> {
    let text = fetch_text(client, sums_url).await.ok()?;
    checksum_for(&text, asset)
}

/// Comprueba el SHA-256 descargado contra el esperado (si se pudo obtener).
async fn verify_checksum(
    client: &Client,
    sums_url: &str,
    asset: &str,
    digest: &str,
) -> Result<(), String> {
    if let Some(expected) = expected_checksum(client, sums_url, asset).await
        && !expected.eq_ignore_ascii_case(digest)
    {
        return Err("La verificación de integridad falló (el SHA-256 no coincide).".to_string());
    }
    Ok(())
}

/// Busca el hash de un asset en un archivo de sumas de control.
fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let hash = fields.next()?;
        let name = fields.next()?.trim_start_matches('*');
        (name == asset).then(|| hash.to_lowercase())
    })
}

/// Ejecuta `bin <arg>` y devuelve `None` si no arranca o no imprime nada.
async fn probe_binary(bin: &Path, args: &[&str]) -> Option<String> {
    probe_command(engine::command(bin), args).await
}

/// Ejecuta un comando ya preparado y devuelve su salida si termina bien.
async fn probe_command(mut command: Command, args: &[&str]) -> Option<String> {
    let output = command
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!stdout.is_empty()).then_some(stdout)
}

/// Extrae la versión de yt-dlp (`--version`).
///
/// Se lanza como lo lanzaría una descarga: en Android el fichero es un *zipapp*
/// y lo tiene que leer el intérprete de Python del APK.
async fn probe_ytdlp(bin: &Path) -> Option<String> {
    let raw = probe_command(
        engine::ytdlp_command_for(bin),
        &["--ignore-config", "--version"],
    )
    .await?;
    Some(raw.lines().next().unwrap_or_default().trim().to_string())
}

/// Extrae la versión de ffmpeg (`-version`, tercer token de la primera línea).
async fn probe_ffmpeg(bin: &Path) -> Option<String> {
    let raw = probe_binary(bin, &["-version"]).await?;
    let first_line = raw.lines().next()?;
    first_line.split_whitespace().nth(2).map(str::to_string)
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

/// Ruta temporal de una descarga. En Windows conserva la extensión `.exe`
/// para poder ejecutarla al verificarla.
fn download_temp_path(dir: &Path, base: &str) -> PathBuf {
    if cfg!(windows) {
        dir.join(format!("{base}.download.exe"))
    } else {
        dir.join(format!("{base}.download"))
    }
}

// ---------------------------------------------------------------------------
// yt-dlp
// ---------------------------------------------------------------------------

/// Nombre del asset de yt-dlp para la plataforma actual, si existe binario oficial.
fn ytdlp_asset_for_platform() -> Option<&'static str> {
    ytdlp_asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Nombre del asset de yt-dlp para un par sistema/arquitectura (función pura).
///
/// Solo cubre los assets publicados como binario suelto. `yt-dlp_linux_armv7l`
/// únicamente se publica dentro de un `.zip`, así que no se ofrece.
fn ytdlp_asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("yt-dlp.exe"),
        ("windows", "x86") => Some("yt-dlp_x86.exe"),
        ("windows", "aarch64") => Some("yt-dlp_arm64.exe"),
        ("linux", "x86_64") => Some("yt-dlp_linux"),
        ("linux", "aarch64") => Some("yt-dlp_linux_aarch64"),
        ("android", _) => None,
        ("macos", "x86_64") | ("macos", "aarch64") => Some("yt-dlp_macos"),
        _ => None,
    }
}

/// URL de descarga de un asset de yt-dlp dentro de una release.
fn ytdlp_asset_url(version: &str, asset: &str) -> String {
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

/// Última etiqueta publicada de un repositorio de GitHub.
async fn latest_tag(client: &Client, api: &str) -> Result<String, String> {
    let text = fetch_text(client, api).await?;
    parse_latest_tag(&text).ok_or_else(|| "GitHub no devolvió una versión válida.".to_string())
}

/// Consulta la última versión publicada de yt-dlp.
async fn latest_ytdlp_version(client: &Client) -> Result<String, String> {
    latest_tag(client, YTDLP_RELEASE_API).await
}

/// Comprueba la versión instalada de yt-dlp frente a la última publicada.
pub async fn check_ytdlp() -> Result<UpdateCheck, String> {
    let client = client()?;
    let latest = latest_ytdlp_version(&client).await?;
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

/// Descarga el binario de yt-dlp a `temp` y lo verifica. Devuelve su versión.
async fn download_ytdlp(
    app: &AppHandle,
    client: &Client,
    version: &str,
    asset: &str,
    temp: &Path,
) -> Result<String, String> {
    let url = ytdlp_asset_url(version, asset);
    let digest = stream_download(app, ENGINE_YTDLP, client, &url, temp).await?;

    emit_phase(app, ENGINE_YTDLP, UpdatePhase::Verifying, None, None);
    verify_checksum(
        client,
        &ytdlp_asset_url(version, "SHA2-256SUMS"),
        asset,
        &digest,
    )
    .await?;

    make_executable(temp)?;
    probe_ytdlp(temp)
        .await
        .ok_or_else(|| "El binario descargado no se pudo ejecutar en esta plataforma.".to_string())
}

/// Descarga e instala la última versión de yt-dlp.
pub async fn update_ytdlp(app: &AppHandle) -> Result<UpdateCheck, String> {
    let _guard = begin_update(&YTDLP_UPDATING)?;
    let client = client()?;

    emit_phase(app, ENGINE_YTDLP, UpdatePhase::Checking, None, None);
    let latest = latest_ytdlp_version(&client).await.inspect_err(|e| {
        emit_phase(app, ENGINE_YTDLP, UpdatePhase::Error, None, Some(e.clone()));
    })?;

    let asset = ytdlp_asset_for_platform().ok_or_else(|| {
        let message = format!(
            "No hay binario oficial de yt-dlp para {} {}.",
            std::env::consts::OS,
            std::env::consts::ARCH
        );
        emit_phase(
            app,
            ENGINE_YTDLP,
            UpdatePhase::Error,
            None,
            Some(message.clone()),
        );
        message
    })?;

    let dir = managed_dir().await?;
    let target = engine::managed_path("yt-dlp")
        .ok_or_else(|| "No se pudo resolver la ruta del binario gestionado.".to_string())?;
    let temp = download_temp_path(&dir, "yt-dlp");

    let installed = match download_ytdlp(app, &client, &latest, asset, &temp).await {
        Ok(version) => version,
        Err(err) => return fail(app, ENGINE_YTDLP, &temp, err).await,
    };

    emit_phase(app, ENGINE_YTDLP, UpdatePhase::Installing, None, None);
    if let Err(err) = tokio::fs::rename(&temp, &target).await {
        let message = format!("No se pudo instalar yt-dlp (¿está en uso?): {err}");
        return fail(app, ENGINE_YTDLP, &temp, message).await;
    }

    emit_phase(
        app,
        ENGINE_YTDLP,
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

// ---------------------------------------------------------------------------
// ffmpeg (builds estáticos de BtbN/FFmpeg-Builds)
// ---------------------------------------------------------------------------

/// Cómo viene empaquetado el asset de ffmpeg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveKind {
    TarXz,
    Zip,
}

/// Asset de ffmpeg para la plataforma actual.
fn ffmpeg_asset_for_platform() -> Option<&'static str> {
    ffmpeg_asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Asset de ffmpeg para un par sistema/arquitectura (función pura). BtbN solo
/// publica builds para Linux y Windows.
fn ffmpeg_asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("ffmpeg-master-latest-linux64-gpl.tar.xz"),
        ("linux", "aarch64") => Some("ffmpeg-master-latest-linuxarm64-gpl.tar.xz"),
        ("windows", "x86_64") => Some("ffmpeg-master-latest-win64-gpl.zip"),
        ("windows", "aarch64") => Some("ffmpeg-master-latest-winarm64-gpl.zip"),
        ("android", _) => None,
        _ => None,
    }
}

/// Formato del archivo a partir de su extensión.
fn archive_kind_for(asset: &str) -> ArchiveKind {
    if asset.ends_with(".zip") {
        ArchiveKind::Zip
    } else {
        ArchiveKind::TarXz
    }
}

/// URL de descarga de un asset de ffmpeg (la release `latest` es móvil).
fn ffmpeg_asset_url(asset: &str) -> String {
    format!("{FFMPEG_DOWNLOAD_BASE}/{asset}")
}

/// `updated_at` del asset concreto dentro del JSON de la release.
fn asset_updated_at(json: &str, asset: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    value
        .get("assets")?
        .as_array()?
        .iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(asset))
        .and_then(|a| a.get("updated_at"))
        .and_then(|u| u.as_str())
        .map(str::to_string)
}

/// Extrae la fecha de compilación (`YYYYMMDD`) de la versión de un build de BtbN,
/// p. ej. `N-127564-g3c92f003c7-20261015` → `20261015`.
fn build_date_from_version(version: &str) -> Option<u32> {
    let date = version.rsplit('-').next()?;
    (date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()))
        .then(|| date.parse().ok())
        .flatten()
}

/// Fecha (`YYYYMMDD`) de un *timestamp* ISO-8601 (`2026-10-15T13:24:00Z`).
fn date_from_timestamp(timestamp: &str) -> Option<u32> {
    let digits: String = timestamp
        .chars()
        .filter(char::is_ascii_digit)
        .take(8)
        .collect();
    (digits.len() == 8).then(|| digits.parse().ok()).flatten()
}

/// ¿Hay que (re)instalar ffmpeg?
///
/// - Si no aparece ningún ffmpeg, sí (descarga).
/// - Si el que hay **no se puede ejecutar** (bibliotecas incompatibles, binario
///   roto…), también: un ffmpeg que no arranca no sirve para nada y la app
///   quedaría sin conversión ni postprocesado.
/// - Si el que hay es la copia gestionada, solo si el build remoto es más nuevo
///   (o si el instalado no reporta una fecha reconocible, para poder reinstalar).
/// - Si es del sistema (o forzado por variable de entorno) y funciona, se respeta.
fn ffmpeg_update_available(
    source: Option<Source>,
    installed_version: Option<&str>,
    remote_timestamp: Option<&str>,
) -> bool {
    match source {
        None => true,
        Some(Source::Sidecar) if cfg!(target_os = "android") => false,
        Some(_) if installed_version.is_none() => true,
        Some(Source::Managed) => {
            match (
                installed_version.and_then(build_date_from_version),
                remote_timestamp.and_then(date_from_timestamp),
            ) {
                (Some(installed), Some(remote)) => remote > installed,
                _ => true,
            }
        }
        Some(_) => false,
    }
}

/// Comprueba si conviene descargar o actualizar la copia gestionada de ffmpeg.
pub async fn check_ffmpeg() -> Result<UpdateCheck, String> {
    // En Android el ffmpeg viene **incluido con la aplicación** (viaja junto a los
    // otros motores en `jniLibs`): no se descarga ni se actualiza por separado, e
    // igual que quickjs, no hay build de escritorio que instalar. Antes esto se
    // resolvía como un error («No hay builds de ffmpeg para android»), que se veía
    // en Ajustes aunque ffmpeg funcionara perfectamente: eso era el «error
    // ficticio» del actualizador.
    if cfg!(target_os = "android") {
        let current = crate::ytdlp::ffmpeg_version().await;
        return Ok(UpdateCheck {
            current,
            latest: None,
            update_available: false,
        });
    }

    let client = client()?;
    let asset = ffmpeg_asset_for_platform().ok_or_else(unsupported_ffmpeg)?;
    let json = fetch_text(&client, FFMPEG_RELEASE_API).await?;
    let remote_timestamp = asset_updated_at(&json, asset);
    let current = crate::ytdlp::ffmpeg_version().await;
    let update_available = ffmpeg_update_available(
        engine::ffmpeg().map(|(_, source)| source),
        current.as_deref(),
        remote_timestamp.as_deref(),
    );

    Ok(UpdateCheck {
        current,
        // Para ffmpeg no hay versión semántica: la release es móvil y se identifica
        // por la fecha de compilación.
        latest: remote_timestamp
            .as_deref()
            .map(|ts| ts.chars().take(10).collect()),
        update_available,
    })
}

/// Mensaje de «ffmpeg no se descarga aquí» para las plataformas que lo traen
/// incluido con la aplicación (Android).
fn bundled_ffmpeg() -> String {
    "ffmpeg va incluido con la aplicación, no necesita descargarse.".to_string()
}

fn unsupported_ffmpeg() -> String {
    format!(
        "No hay builds de ffmpeg para {} {}. Usa el ffmpeg del sistema.",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Descarga el archivo de ffmpeg y verifica su SHA-256.
async fn download_ffmpeg_archive(
    app: &AppHandle,
    client: &Client,
    asset: &str,
    dest: &Path,
) -> Result<(), String> {
    let digest =
        stream_download(app, ENGINE_FFMPEG, client, &ffmpeg_asset_url(asset), dest).await?;
    emit_phase(app, ENGINE_FFMPEG, UpdatePhase::Verifying, None, None);

    let sums_url = format!("{FFMPEG_DOWNLOAD_BASE}/checksums.sha256");
    verify_checksum(client, &sums_url, asset, &digest).await
}

/// Extrae `ffmpeg` y `ffprobe` del archivo descargado (bloqueante, en hilo aparte).
fn extract_binaries(
    archive: &Path,
    kind: ArchiveKind,
    out_ffmpeg: &Path,
    out_ffprobe: &Path,
) -> Result<(), String> {
    match kind {
        ArchiveKind::Zip => {
            let file = StdFile::open(archive)
                .map_err(|e| format!("No se pudo abrir el archivo de ffmpeg: {e}"))?;
            extract_from_zip(file, out_ffmpeg, out_ffprobe)?;
        }
        ArchiveKind::TarXz => {
            let file = StdFile::open(archive)
                .map_err(|e| format!("No se pudo abrir el archivo de ffmpeg: {e}"))?;
            let mut reader = BufReader::new(file);
            // Descomprimir a un .tar temporal (no cabe entero en memoria con holgura).
            let mut tar_tmp = archive.to_path_buf();
            tar_tmp.set_extension("tar");
            {
                let out = StdFile::create(&tar_tmp)
                    .map_err(|e| format!("No se pudo crear el archivo temporal: {e}"))?;
                let mut writer = BufWriter::new(out);
                lzma_rs::xz_decompress(&mut reader, &mut writer)
                    .map_err(|e| format!("No se pudo descomprimir ffmpeg: {e}"))?;
                writer
                    .flush()
                    .map_err(|e| format!("No se pudo escribir el archivo temporal: {e}"))?;
            }

            let tar_file = StdFile::open(&tar_tmp)
                .map_err(|e| format!("No se pudo releer el archivo temporal: {e}"))?;
            let result = extract_from_tar(BufReader::new(tar_file), out_ffmpeg, out_ffprobe);
            let _ = std::fs::remove_file(&tar_tmp);
            result?;
        }
    }

    for path in [out_ffmpeg, out_ffprobe] {
        if !path.is_file() {
            return Err(format!(
                "El archivo de ffmpeg no contenía {}.",
                path.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
    }
    Ok(())
}

/// Copia `ffmpeg`/`ffprobe` desde un `.zip` al destino.
fn extract_from_zip<R: Read + Seek>(
    reader: R,
    out_ffmpeg: &Path,
    out_ffprobe: &Path,
) -> Result<(), String> {
    let mut archive =
        zip::ZipArchive::new(reader).map_err(|e| format!("El .zip de ffmpeg es ilegible: {e}"))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|e| format!("Entrada ilegible en el .zip de ffmpeg: {e}"))?;
        let name = Path::new(entry.name())
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let Some(target) = binary_target(&name, out_ffmpeg, out_ffprobe) else {
            continue;
        };
        let mut out =
            StdFile::create(target).map_err(|e| format!("No se pudo extraer el binario: {e}"))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("No se pudo extraer el binario: {e}"))?;
    }
    Ok(())
}

/// Copia `ffmpeg`/`ffprobe` desde un tar (ya descomprimido) al destino.
fn extract_from_tar<R: Read>(
    reader: R,
    out_ffmpeg: &Path,
    out_ffprobe: &Path,
) -> Result<(), String> {
    let mut archive = tar::Archive::new(reader);
    let entries = archive
        .entries()
        .map_err(|e| format!("El tar de ffmpeg es ilegible: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Entrada ilegible en el tar: {e}"))?;
        let name = entry
            .path()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_default();
        let Some(target) = binary_target(&name, out_ffmpeg, out_ffprobe) else {
            continue;
        };
        let mut out =
            StdFile::create(target).map_err(|e| format!("No se pudo extraer el binario: {e}"))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("No se pudo extraer el binario: {e}"))?;
    }
    Ok(())
}

/// Traduce el nombre de una entrada del archivo al destino que le corresponde.
fn binary_target<'a>(name: &str, out_ffmpeg: &'a Path, out_ffprobe: &'a Path) -> Option<&'a Path> {
    match name {
        "ffmpeg" | "ffmpeg.exe" => Some(out_ffmpeg),
        "ffprobe" | "ffprobe.exe" => Some(out_ffprobe),
        _ => None,
    }
}

/// Descarga e instala la última versión de ffmpeg (ffmpeg + ffprobe).
pub async fn update_ffmpeg(app: &AppHandle) -> Result<UpdateCheck, String> {
    let _guard = begin_update(&FFMPEG_UPDATING)?;

    emit_phase(app, ENGINE_FFMPEG, UpdatePhase::Checking, None, None);
    if cfg!(target_os = "android") {
        // Igual que en `check_ffmpeg`: en Android no hay nada que descargar.
        let current = crate::ytdlp::ffmpeg_version().await;
        emit_phase(
            app,
            ENGINE_FFMPEG,
            UpdatePhase::Done,
            None,
            Some(bundled_ffmpeg()),
        );
        return Ok(UpdateCheck {
            current,
            latest: None,
            update_available: false,
        });
    }

    let client = client()?;
    let asset = ffmpeg_asset_for_platform().ok_or_else(|| {
        let message = unsupported_ffmpeg();
        emit_phase(
            app,
            ENGINE_FFMPEG,
            UpdatePhase::Error,
            None,
            Some(message.clone()),
        );
        message
    })?;

    let json = fetch_text(&client, FFMPEG_RELEASE_API)
        .await
        .inspect_err(|e| {
            emit_phase(
                app,
                ENGINE_FFMPEG,
                UpdatePhase::Error,
                None,
                Some(e.clone()),
            );
        })?;
    let remote_timestamp = asset_updated_at(&json, asset);

    let dir = managed_dir().await?;
    let archive = dir.join(asset);
    let temp_ffmpeg = download_temp_path(&dir, "ffmpeg");
    let temp_ffprobe = download_temp_path(&dir, "ffprobe");

    if let Err(err) = download_ffmpeg_archive(app, &client, asset, &archive).await {
        let _ = tokio::fs::remove_file(&archive).await;
        return fail(app, ENGINE_FFMPEG, &archive, err).await;
    }

    // Descomprimir es costoso (cientos de MB): fuera del hilo async.
    emit_phase(app, ENGINE_FFMPEG, UpdatePhase::Installing, None, None);
    let extract = {
        let archive = archive.clone();
        let temp_ffmpeg = temp_ffmpeg.clone();
        let temp_ffprobe = temp_ffprobe.clone();
        let kind = archive_kind_for(asset);
        tokio::task::spawn_blocking(move || {
            extract_binaries(&archive, kind, &temp_ffmpeg, &temp_ffprobe)
        })
        .await
        .map_err(|e| format!("La extracción de ffmpeg falló: {e}"))
    };

    let _ = tokio::fs::remove_file(&archive).await;
    let failed = match extract {
        Ok(Ok(())) => None,
        Ok(Err(err)) | Err(err) => Some(err),
    };
    let installed = match failed {
        Some(err) => {
            cleanup(&[&temp_ffmpeg, &temp_ffprobe]).await;
            return fail(app, ENGINE_FFMPEG, &temp_ffmpeg, err).await;
        }
        None => match verify_ffmpeg_binaries(&temp_ffmpeg, &temp_ffprobe).await {
            Ok(version) => version,
            Err(err) => {
                cleanup(&[&temp_ffmpeg, &temp_ffprobe]).await;
                return fail(app, ENGINE_FFMPEG, &temp_ffmpeg, err).await;
            }
        },
    };

    // Instalación atómica: ffmpeg y ffprobe juntos.
    for (temp, name) in [(&temp_ffmpeg, "ffmpeg"), (&temp_ffprobe, "ffprobe")] {
        let target = engine::managed_path(name)
            .ok_or_else(|| "No se pudo resolver la ruta del binario gestionado.".to_string())?;
        if let Err(err) = tokio::fs::rename(temp, &target).await {
            cleanup(&[&temp_ffmpeg, &temp_ffprobe]).await;
            let message = format!("No se pudo instalar {name} (¿está en uso?): {err}");
            emit_phase(
                app,
                ENGINE_FFMPEG,
                UpdatePhase::Error,
                None,
                Some(message.clone()),
            );
            return Err(message);
        }
    }

    emit_phase(
        app,
        ENGINE_FFMPEG,
        UpdatePhase::Done,
        None,
        Some(format!("ffmpeg actualizado a {installed}")),
    );

    Ok(UpdateCheck {
        current: Some(installed),
        latest: remote_timestamp
            .as_deref()
            .map(|ts| ts.chars().take(10).collect()),
        update_available: false,
    })
}

/// Da permisos de ejecución a los binarios extraídos y comprueba que ffmpeg corre.
async fn verify_ffmpeg_binaries(ffmpeg: &Path, ffprobe: &Path) -> Result<String, String> {
    make_executable(ffmpeg)?;
    make_executable(ffprobe)?;
    probe_ffmpeg(ffmpeg)
        .await
        .ok_or_else(|| "El ffmpeg descargado no se pudo ejecutar en esta plataforma.".to_string())
}

/// Emite el error, limpia el temporal y lo propaga.
async fn fail(
    app: &AppHandle,
    engine: &str,
    temp: &Path,
    error: String,
) -> Result<UpdateCheck, String> {
    let _ = tokio::fs::remove_file(temp).await;
    emit_phase(app, engine, UpdatePhase::Error, None, Some(error.clone()));
    Err(error)
}

/// Borra archivos temporales ignorando los errores.
async fn cleanup(paths: &[&Path]) {
    for path in paths {
        let _ = tokio::fs::remove_file(path).await;
    }
}

/// Carpeta de binarios gestionados, creada si hace falta.
async fn managed_dir() -> Result<PathBuf, String> {
    let dir = engine::managed_bin_dir()
        .ok_or_else(|| "No se pudo resolver la carpeta de binarios gestionados.".to_string())?
        .to_path_buf();
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| format!("No se pudo crear la carpeta de binarios: {e}"))?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Runtime JS (quickjs-ng)
// ---------------------------------------------------------------------------

/// Asset de quickjs-ng para la plataforma actual.
fn quickjs_asset_for_platform() -> Option<&'static str> {
    quickjs_asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

/// Asset de quickjs-ng para un par sistema/arquitectura (función pura).
///
/// Solo se ofrecen los binarios sueltos publicados en sus *releases*. No hay
/// build para Windows arm64, así que esa plataforma queda fuera.
fn quickjs_asset_for(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("qjs-linux-x86_64"),
        ("linux", "x86") => Some("qjs-linux-x86"),
        ("linux", "aarch64") => Some("qjs-linux-aarch64"),
        ("linux", "arm") => Some("qjs-linux-armv7"),
        ("linux", "riscv64") => Some("qjs-linux-riscv64"),
        ("windows", "x86_64") => Some("qjs-windows-x86_64.exe"),
        ("windows", "x86") => Some("qjs-windows-x86.exe"),
        ("macos", "x86_64") => Some("qjs-darwin-x86_64"),
        ("macos", "aarch64") => Some("qjs-darwin-arm64"),
        ("android", _) => None,
        _ => None,
    }
}

/// URL de descarga de un asset de quickjs-ng dentro de una release.
fn quickjs_asset_url(tag: &str, asset: &str) -> String {
    format!("{QUICKJS_DOWNLOAD_BASE}/{tag}/{asset}")
}

fn unsupported_quickjs() -> String {
    if cfg!(target_os = "android") {
        return "quickjs va incluido con la aplicación, no necesita descargarse.".to_string();
    }
    format!(
        "No hay binario de quickjs para {} {}. Usa el node o deno del sistema.",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Versión de quickjs. `qjs -v` imprime solo el número; si esa forma no diera
/// nada (otras variantes), se recurre a `--help`, que incluye la cabecera
/// «QuickJS-ng version X».
async fn probe_quickjs(bin: &Path) -> Option<String> {
    if let Some(raw) = probe_binary(bin, &["-v"]).await {
        let version = raw.lines().next().unwrap_or_default().trim().to_string();
        if !version.is_empty() {
            return Some(version);
        }
    }

    let output = engine::command(bin)
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find(|line| line.contains("QuickJS"))
        .map(|line| line.trim().to_string())
}

/// Versión del quickjs gestionado, si está instalado y arranca.
async fn managed_quickjs_version() -> Option<String> {
    let path = crate::ytdlp::managed_js_runtime_path()?;
    probe_quickjs(&path).await
}

/// ¿Hay un runtime JS del `PATH` (deno/node/bun/quickjs)?
fn system_js_runtime_available() -> bool {
    if cfg!(target_os = "android") {
        return false;
    }
    matches!(
        crate::ytdlp::resolve_js_runtime().map(|runtime| runtime.source),
        Some(Source::System)
    )
}

/// Comprueba si conviene descargar el runtime JS gestionado (quickjs).
///
/// Solo se propone cuando **no hay ningún runtime** disponible: si el sistema ya
/// trae node/deno/bun/quickjs, se respeta. Con la copia gestionada presente, se
/// actualiza si quickjs-ng publica una versión más nueva.
pub async fn check_js_runtime() -> Result<UpdateCheck, String> {
    let client = client()?;
    let tag = latest_tag(&client, QUICKJS_RELEASE_API).await?;
    let current = managed_quickjs_version().await;

    // Un override manual (`DOWNLINK_JS_RUNTIME`) o un runtime del sistema
    // relevan de la copia gestionada.
    let update_available = if cfg!(target_os = "android") {
        false
    } else {
        if std::env::var_os("DOWNLINK_JS_RUNTIME").is_some() || system_js_runtime_available() {
            false
        } else {
            match &current {
                None => true,
                Some(installed) => is_newer(installed, tag.trim_start_matches('v')),
            }
        }
    };

    Ok(UpdateCheck {
        current,
        latest: Some(tag),
        update_available,
    })
}

/// Descarga e instala el runtime JS gestionado (quickjs-ng).
///
/// quickjs-ng no publica archivo de sumas de control, así que la integridad se
/// comprueba **ejecutando** el binario descargado (`qjs -v`), igual que hace
/// yt-dlp con su propia comprobación de arranque.
pub async fn update_js_runtime(app: &AppHandle) -> Result<UpdateCheck, String> {
    let _guard = begin_update(&QUICKJS_UPDATING)?;
    let client = client()?;

    emit_phase(app, ENGINE_QUICKJS, UpdatePhase::Checking, None, None);
    let tag = latest_tag(&client, QUICKJS_RELEASE_API)
        .await
        .inspect_err(|e| {
            emit_phase(
                app,
                ENGINE_QUICKJS,
                UpdatePhase::Error,
                None,
                Some(e.clone()),
            );
        })?;

    let asset = quickjs_asset_for_platform().ok_or_else(|| {
        let message = unsupported_quickjs();
        emit_phase(
            app,
            ENGINE_QUICKJS,
            UpdatePhase::Error,
            None,
            Some(message.clone()),
        );
        message
    })?;

    let dir = managed_dir().await?;
    let target = engine::managed_path(QUICKJS_BIN)
        .ok_or_else(|| "No se pudo resolver la ruta del binario gestionado.".to_string())?;
    let temp = download_temp_path(&dir, "qjs");

    let url = quickjs_asset_url(&tag, asset);
    if let Err(err) = stream_download(app, ENGINE_QUICKJS, &client, &url, &temp).await {
        let _ = tokio::fs::remove_file(&temp).await;
        return fail(app, ENGINE_QUICKJS, &temp, err).await;
    }

    emit_phase(app, ENGINE_QUICKJS, UpdatePhase::Verifying, None, None);
    if let Err(err) = make_executable(&temp) {
        return fail(app, ENGINE_QUICKJS, &temp, err).await;
    }
    let Some(installed) = probe_quickjs(&temp).await else {
        let message = "El quickjs descargado no se pudo ejecutar en esta plataforma.".to_string();
        return fail(app, ENGINE_QUICKJS, &temp, message).await;
    };

    emit_phase(app, ENGINE_QUICKJS, UpdatePhase::Installing, None, None);
    if let Err(err) = tokio::fs::rename(&temp, &target).await {
        let message = format!("No se pudo instalar el runtime JS (¿está en uso?): {err}");
        return fail(app, ENGINE_QUICKJS, &temp, message).await;
    }

    emit_phase(
        app,
        ENGINE_QUICKJS,
        UpdatePhase::Done,
        None,
        Some(format!("runtime JS actualizado a {installed}")),
    );

    Ok(UpdateCheck {
        current: Some(installed),
        latest: Some(tag),
        update_available: false,
    })
}

// ---------------------------------------------------------------------------
// Actualización automática al arrancar
// ---------------------------------------------------------------------------

/// Actualiza los motores al arrancar según los ajustes. Nunca propaga errores:
/// una actualización fallida no debe impedir usar la app.
pub async fn auto_update(app: &AppHandle) {
    let (update_ytdlp_enabled, download_ffmpeg_enabled, download_js_runtime_enabled) = {
        let state = app.state::<SettingsState>();
        let settings = state.0.lock().await;
        (
            settings.auto_update_ytdlp,
            settings.auto_download_ffmpeg,
            settings.auto_download_js_runtime,
        )
    };

    // yt-dlp: se mantiene al día. Si el usuario forzó una ruta, se respeta.
    if update_ytdlp_enabled && std::env::var_os("DOWNLINK_YTDLP").is_none() {
        match check_ytdlp().await {
            Ok(result) if result.update_available => {
                if let Err(err) = update_ytdlp(app).await {
                    eprintln!("[downlink] auto-update de yt-dlp: {err}");
                }
            }
            Ok(_) => {}
            Err(err) => eprintln!("[downlink] no se pudo comprobar yt-dlp: {err}"),
        }
    }

    // ffmpeg: solo la descarga inicial cuando falta (los builds pesan ~150 MB y
    // se regeneran a diario, así que no se refresca solo).
    if download_ffmpeg_enabled
        && engine::ffmpeg().is_none()
        && let Err(err) = update_ffmpeg(app).await
    {
        eprintln!("[downlink] no se pudo descargar ffmpeg: {err}");
    }

    // Runtime JS: solo si no hay ninguno (quickjs pesa ~2,5 MB). Así YouTube
    // funciona sin obligar a instalar Node.js a mano. No se refresca solo: si ya
    // hay un runtime (del sistema o el gestionado), se respeta.
    if download_js_runtime_enabled
        && std::env::var_os("DOWNLINK_JS_RUNTIME").is_none()
        && crate::ytdlp::resolve_js_runtime().is_none()
        && let Err(err) = update_js_runtime(app).await
    {
        eprintln!("[downlink] no se pudo descargar el runtime JS: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_tmp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("downlink-updater-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hay_asset_de_ytdlp_para_plataformas_habituales() {
        assert_eq!(ytdlp_asset_for("linux", "x86_64"), Some("yt-dlp_linux"));
        assert_eq!(
            ytdlp_asset_for("linux", "aarch64"),
            Some("yt-dlp_linux_aarch64")
        );
        assert_eq!(ytdlp_asset_for("windows", "x86_64"), Some("yt-dlp.exe"));
        assert_eq!(ytdlp_asset_for("macos", "aarch64"), Some("yt-dlp_macos"));
        assert_eq!(ytdlp_asset_for("solaris", "sparc"), None);
        // linux/arm solo se publica empaquetado en .zip: no se ofrece.
        assert_eq!(ytdlp_asset_for("linux", "arm"), None);
    }

    #[test]
    fn el_asset_actual_de_ytdlp_es_el_de_esta_plataforma() {
        let asset = ytdlp_asset_for_platform().expect("esta plataforma debe tener binario");
        assert!(asset.starts_with("yt-dlp"));
    }

    #[test]
    fn hay_asset_de_ffmpeg_para_linux_y_windows() {
        assert_eq!(
            ffmpeg_asset_for("linux", "x86_64"),
            Some("ffmpeg-master-latest-linux64-gpl.tar.xz")
        );
        assert_eq!(
            ffmpeg_asset_for("windows", "aarch64"),
            Some("ffmpeg-master-latest-winarm64-gpl.zip")
        );
        // BtbN no publica macOS.
        assert_eq!(ffmpeg_asset_for("macos", "aarch64"), None);
        assert_eq!(ffmpeg_asset_for("android", "aarch64"), None);
    }

    #[test]
    fn hay_asset_de_quickjs_para_plataformas_habituales() {
        assert_eq!(
            quickjs_asset_for("linux", "x86_64"),
            Some("qjs-linux-x86_64")
        );
        assert_eq!(
            quickjs_asset_for("linux", "aarch64"),
            Some("qjs-linux-aarch64")
        );
        assert_eq!(
            quickjs_asset_for("windows", "x86_64"),
            Some("qjs-windows-x86_64.exe")
        );
        // quickjs-ng usa "arm64" para macOS y no publica Windows arm64.
        assert_eq!(
            quickjs_asset_for("macos", "aarch64"),
            Some("qjs-darwin-arm64")
        );
        assert_eq!(quickjs_asset_for("windows", "aarch64"), None);
        assert_eq!(quickjs_asset_for("solaris", "sparc"), None);
        assert_eq!(quickjs_asset_for("android", "aarch64"), None);
    }

    #[test]
    fn el_asset_actual_de_quickjs_es_el_de_esta_plataforma() {
        // Esta plataforma debe tener quickjs (CI corre en linux y windows x86_64).
        let asset = quickjs_asset_for_platform().expect("esta plataforma debe tener quickjs");
        assert!(asset.starts_with("qjs-"));
    }

    #[test]
    fn construye_la_url_del_asset_de_quickjs() {
        assert_eq!(
            quickjs_asset_url("v0.17.0", "qjs-linux-x86_64"),
            "https://github.com/quickjs-ng/quickjs/releases/download/v0.17.0/qjs-linux-x86_64"
        );
    }

    #[test]
    fn detecta_el_formato_del_archivo() {
        assert_eq!(archive_kind_for("x.zip"), ArchiveKind::Zip);
        assert_eq!(archive_kind_for("x.tar.xz"), ArchiveKind::TarXz);
    }

    #[test]
    fn construye_la_url_del_asset_de_ytdlp() {
        assert_eq!(
            ytdlp_asset_url("2025.01.15", "yt-dlp_linux"),
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
    fn lee_la_fecha_del_asset_de_la_release() {
        let json = r#"{"assets":[
            {"name":"otro.zip","updated_at":"2020-01-01T00:00:00Z"},
            {"name":"ffmpeg-master-latest-linux64-gpl.tar.xz","updated_at":"2026-10-15T13:24:00Z"}
        ]}"#;
        assert_eq!(
            asset_updated_at(json, "ffmpeg-master-latest-linux64-gpl.tar.xz").as_deref(),
            Some("2026-10-15T13:24:00Z")
        );
        assert_eq!(asset_updated_at(json, "no-existe.zip"), None);
    }

    #[test]
    fn compara_versiones_de_ytdlp_por_componentes() {
        assert!(is_newer("2025.01.15", "2025.02.01"));
        assert!(is_newer("2024.12.31", "2025.01.01"));
        assert!(is_newer("2025.01.15", "2025.01.15.123456"));
        assert!(!is_newer("2025.02.01", "2025.01.15"));
        assert!(!is_newer("2025.01.15", "2025.01.15"));
    }

    #[test]
    fn extrae_la_fecha_de_compilacion_de_ffmpeg() {
        assert_eq!(
            build_date_from_version("N-127564-g3c92f003c7-20261015"),
            Some(20261015)
        );
        // Una versión de distro no lleva fecha de build.
        assert_eq!(build_date_from_version("6.1.1"), None);
    }

    #[test]
    fn convierte_un_timestamp_a_fecha() {
        assert_eq!(date_from_timestamp("2026-10-15T13:24:00Z"), Some(20261015));
        assert_eq!(date_from_timestamp("basura"), None);
    }

    #[test]
    fn decide_cuando_actualizar_ffmpeg() {
        // Sin ffmpeg instalado: siempre hay que descargarlo.
        assert!(ffmpeg_update_available(
            None,
            None,
            Some("2026-10-15T00:00:00Z")
        ));
        // Copia gestionada antigua: sí.
        assert!(ffmpeg_update_available(
            Some(Source::Managed),
            Some("N-1-gabcdef-20260101"),
            Some("2026-10-15T00:00:00Z"),
        ));
        // Copia gestionada al día: no.
        assert!(!ffmpeg_update_available(
            Some(Source::Managed),
            Some("N-1-gabcdef-20261015"),
            Some("2026-10-15T00:00:00Z"),
        ));
        // Copia gestionada sin fecha reconocible: se ofrece reinstalar.
        assert!(ffmpeg_update_available(
            Some(Source::Managed),
            Some("7.1"),
            Some("2026-10-15T00:00:00Z"),
        ));
        // ffmpeg del sistema: se respeta.
        assert!(!ffmpeg_update_available(
            Some(Source::System),
            Some("6.1.1"),
            Some("2026-10-15T00:00:00Z"),
        ));
        // ffmpeg del sistema que no arranca (no reporta versión): se ofrece
        // la copia gestionada, porque ese binario no sirve.
        assert!(ffmpeg_update_available(
            Some(Source::System),
            None,
            Some("2026-10-15T00:00:00Z"),
        ));
    }

    #[test]
    fn extrae_el_hash_del_asset() {
        let sums = "\
aaaa1111  yt-dlp_linux\n\
bbbb2222  ffmpeg-master-latest-linux64-gpl.tar.xz\n\
cccc3333 *yt-dlp.exe\n";
        assert_eq!(
            checksum_for(sums, "yt-dlp_linux").as_deref(),
            Some("aaaa1111")
        );
        assert_eq!(
            checksum_for(sums, "ffmpeg-master-latest-linux64-gpl.tar.xz").as_deref(),
            Some("bbbb2222")
        );
        assert_eq!(
            checksum_for(sums, "yt-dlp.exe").as_deref(),
            Some("cccc3333")
        );
        assert_eq!(checksum_for(sums, "no-existe"), None);
    }

    #[test]
    fn el_temporal_lleva_exe_solo_en_windows() {
        let temp = download_temp_path(Path::new("/tmp/bin"), "yt-dlp");
        let name = temp.file_name().unwrap().to_string_lossy();
        if cfg!(windows) {
            assert_eq!(name, "yt-dlp.download.exe");
        } else {
            assert_eq!(name, "yt-dlp.download");
        }
    }

    #[test]
    fn extrae_binarios_de_un_zip() {
        let dir = unique_tmp_dir("zip");
        let zip_path = dir.join("ffmpeg.zip");
        {
            let mut writer = zip::ZipWriter::new(StdFile::create(&zip_path).unwrap());
            let opts = zip::write::SimpleFileOptions::default();
            writer.start_file("ffmpeg-x/bin/ffmpeg", opts).unwrap();
            writer.write_all(b"soy-ffmpeg").unwrap();
            writer.start_file("ffmpeg-x/bin/ffprobe", opts).unwrap();
            writer.write_all(b"soy-ffprobe").unwrap();
            writer.start_file("ffmpeg-x/README.txt", opts).unwrap();
            writer.write_all(b"texto").unwrap();
            writer.finish().unwrap();
        }

        let out_ffmpeg = dir.join("ffmpeg.download");
        let out_ffprobe = dir.join("ffprobe.download");
        extract_from_zip(StdFile::open(&zip_path).unwrap(), &out_ffmpeg, &out_ffprobe).unwrap();

        assert_eq!(std::fs::read(&out_ffmpeg).unwrap(), b"soy-ffmpeg");
        assert_eq!(std::fs::read(&out_ffprobe).unwrap(), b"soy-ffprobe");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extrae_binarios_de_un_tar() {
        let dir = unique_tmp_dir("tar");
        let mut builder = tar::Builder::new(Vec::new());
        for (path, content) in [
            ("ffmpeg-x/bin/ffmpeg", &b"soy-ffmpeg"[..]),
            ("ffmpeg-x/bin/ffprobe", &b"soy-ffprobe"[..]),
        ] {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder.append_data(&mut header, path, content).unwrap();
        }
        let tar_bytes = builder.into_inner().unwrap();

        let out_ffmpeg = dir.join("ffmpeg.download");
        let out_ffprobe = dir.join("ffprobe.download");
        extract_from_tar(std::io::Cursor::new(tar_bytes), &out_ffmpeg, &out_ffprobe).unwrap();

        assert_eq!(std::fs::read(&out_ffmpeg).unwrap(), b"soy-ffmpeg");
        assert_eq!(std::fs::read(&out_ffprobe).unwrap(), b"soy-ffprobe");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn las_fases_serializan_en_minusculas() {
        let json = serde_json::to_string(&UpdatePhase::Downloading).unwrap();
        assert_eq!(json, "\"downloading\"");
    }

    /// Integración opcional contra un archivo real de BtbN (`.tar.xz` o `.zip`).
    /// No corre por defecto; se activa apuntando a un archivo ya descargado:
    ///
    /// ```text
    /// DOWNLINK_FFMPEG_TEST_ARCHIVE=/ruta/ffmpeg-master-latest-linux64-gpl.tar.xz \
    ///     cargo test -- --ignored extrae_binarios_reales_de_ffmpeg
    /// ```
    #[test]
    #[ignore = "requiere un archivo real; ver DOWNLINK_FFMPEG_TEST_ARCHIVE"]
    fn extrae_binarios_reales_de_ffmpeg() {
        let archive = std::env::var("DOWNLINK_FFMPEG_TEST_ARCHIVE")
            .expect("define DOWNLINK_FFMPEG_TEST_ARCHIVE con un archivo de BtbN");
        let archive = PathBuf::from(archive);
        let dir = unique_tmp_dir("real");
        let out_ffmpeg = dir.join("ffmpeg.download");
        let out_ffprobe = dir.join("ffprobe.download");

        extract_binaries(
            &archive,
            archive_kind_for(&archive.to_string_lossy()),
            &out_ffmpeg,
            &out_ffprobe,
        )
        .unwrap();

        // Los binarios estáticos reales pesan decenas de MB.
        assert!(std::fs::metadata(&out_ffmpeg).unwrap().len() > 1_000_000);
        assert!(std::fs::metadata(&out_ffprobe).unwrap().len() > 1_000_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// La UI se suscribe a `engine-update` cuando ya monta, y la descarga de
    /// arranque puede haber empezado antes: este estado es lo que la permite
    /// mostrar la barra aunque haya llegado tarde a los primeros eventos.
    #[test]
    fn recuerda_el_ultimo_progreso_de_motor() {
        record_progress(&UpdateProgress {
            engine: "yt-dlp".to_string(),
            phase: UpdatePhase::Downloading,
            percent: Some(42.0),
            message: None,
        });

        let last = last_progress().expect("debe recordar el último progreso");
        assert_eq!(last.engine, "yt-dlp");
        assert_eq!(last.phase, UpdatePhase::Downloading);
        assert_eq!(last.percent, Some(42.0));

        record_progress(&UpdateProgress {
            engine: "yt-dlp".to_string(),
            phase: UpdatePhase::Done,
            percent: Some(100.0),
            message: None,
        });
        assert_eq!(last_progress().unwrap().phase, UpdatePhase::Done);
    }
}
