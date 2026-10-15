mod settings;
mod ytdlp;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tauri::{Emitter, Manager};
use tokio::sync::Mutex;

use settings::{Settings, SettingsState};

/// Trabajos activos: job_id → proceso yt-dlp en marcha.
struct Jobs(ytdlp::Jobs);

/// Generador de ids incremental para los trabajos.
struct JobCounter(AtomicU64);

/// Workaround: WebKitGTK con drivers NVIDIA propietarios choca en Wayland con
/// "Error 71 (Protocol error)" al usar el renderer dmabuf. Lo deshabilitamos al
/// arrancar (antes de inicializar GTK), salvo que el usuario ya lo haya definido.
#[cfg(target_os = "linux")]
fn apply_webkit_workarounds() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: corre al inicio del proceso, antes de init de GTK/WebKit y sin
        // otros threads en ejecución.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
}

/// Analiza un enlace y devuelve sus metadatos sin descargar nada.
#[tauri::command]
async fn probe_url(app: tauri::AppHandle, url: String) -> Result<ytdlp::ProbeInfo, String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    ytdlp::probe(&settings, &url).await
}

/// Inicia una descarga en la carpeta de Descargas del usuario.
/// Devuelve el id del trabajo; el progreso llega por el evento `download-progress`.
#[tauri::command]
async fn start_download(
    app: tauri::AppHandle,
    url: String,
    options: ytdlp::DownloadOptions,
) -> Result<u64, String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    let job_id = app.state::<JobCounter>().0.fetch_add(1, Ordering::SeqCst) + 1;
    let out_dir = app
        .path()
        .download_dir()
        .map_err(|_| "No se encontró la carpeta de Descargas.".to_string())?;
    let jobs = app.state::<Jobs>().0.clone();
    ytdlp::spawn_download(app, job_id, url, out_dir, jobs, settings, options).await?;
    Ok(job_id)
}

/// Cancela una descarga en curso.
#[tauri::command]
async fn cancel_download(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let jobs = app.state::<Jobs>().0.clone();
    if ytdlp::cancel(&jobs, id).await {
        let payload = ytdlp::ProgressPayload {
            job_id: id,
            percent: None,
            downloaded_bytes: None,
            total_bytes: None,
            speed_bps: None,
            eta_secs: None,
            status: "cancelled",
            file_path: None,
            message: None,
        };
        let _ = app.emit(ytdlp::DOWNLOAD_EVENT, payload);
    }
    Ok(())
}

/// Configuración actual.
#[tauri::command]
async fn get_settings(app: tauri::AppHandle) -> Result<Settings, String> {
    Ok(app.state::<SettingsState>().0.lock().await.clone())
}

/// Guarda la configuración y la aplica en memoria.
#[tauri::command]
async fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    settings::save(&app, &settings)?;
    *app.state::<SettingsState>().0.lock().await = settings;
    Ok(())
}

/// Navegadores instalados de los que se pueden leer cookies.
#[tauri::command]
fn detect_browsers() -> Vec<settings::BrowserInfo> {
    settings::detect_browsers()
}

/// Versión de yt-dlp y runtime JS detectado.
#[tauri::command]
async fn engine_info() -> settings::EngineInfo {
    ytdlp::engine_info().await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Jobs(Arc::new(Mutex::new(HashMap::new()))))
        .manage(JobCounter(AtomicU64::new(0)))
        .setup(|app| {
            let settings = settings::load(app.handle());
            app.manage(SettingsState(Mutex::new(settings)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe_url,
            start_download,
            cancel_download,
            get_settings,
            save_settings,
            detect_browsers,
            engine_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
