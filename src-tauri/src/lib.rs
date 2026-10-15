mod queue;
mod settings;
mod ytdlp;

use std::sync::Arc;

use tauri::{Manager, State};

use queue::{JobView, Queue};
use settings::{Settings, SettingsState};
use ytdlp::DownloadOptions;

/// Estado global de la cola de descargas.
struct QueueState(Arc<Queue>);

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

/// Añade una descarga a la cola. Devuelve el id del trabajo; el estado llega por
/// el evento `queue-updated`.
#[tauri::command]
async fn start_download(
    app: tauri::AppHandle,
    url: String,
    title: Option<String>,
    options: DownloadOptions,
) -> Result<u64, String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    let queue = app.state::<QueueState>().0.clone();
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.clone());
    Ok(queue.enqueue(&settings, url, title, options).await)
}

/// Pausa un trabajo en curso.
#[tauri::command]
async fn pause_download(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    app.state::<QueueState>().0.pause(id).await;
    Ok(())
}

/// Reanuda un trabajo pausado.
#[tauri::command]
async fn resume_download(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    app.state::<QueueState>().0.resume(&settings, id).await;
    Ok(())
}

/// Cancela un trabajo.
#[tauri::command]
async fn cancel_download(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    app.state::<QueueState>().0.cancel(id).await;
    Ok(())
}

/// Reordena los trabajos pendientes de la cola.
#[tauri::command]
async fn reorder_queue(app: tauri::AppHandle, ids: Vec<u64>) -> Result<(), String> {
    app.state::<QueueState>().0.reorder(&ids).await;
    Ok(())
}

/// Quita de la cola los trabajos terminados.
#[tauri::command]
async fn clear_finished(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<QueueState>().0.clear_finished().await;
    Ok(())
}

/// Estado actual de la cola (para sincronizar al montar la vista).
#[tauri::command]
async fn get_queue(state: State<'_, QueueState>) -> Result<Vec<JobView>, String> {
    Ok(state.0.snapshot().await)
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
    *app.state::<SettingsState>().0.lock().await = settings.clone();

    let queue = app.state::<QueueState>().0.clone();
    queue.set_concurrency(settings.concurrency as usize).await;
    // Si el usuario subió la concurrencia, arranca los pendientes que quepan.
    queue.pump(settings).await;

    Ok(())
}

/// Navegadores instalados de los que se pueden leer cookies.
#[tauri::command]
fn detect_browsers() -> Vec<settings::BrowserInfo> {
    settings::detect_browsers()
}

/// Versión de yt-dlp, runtime JS y ffmpeg detectados.
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
        .setup(|app| {
            let settings = settings::load(app.handle());
            let concurrency = settings.concurrency as usize;
            app.manage(SettingsState(tokio::sync::Mutex::new(settings)));
            app.manage(QueueState(Arc::new(Queue::new(
                app.handle().clone(),
                concurrency,
            ))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            probe_url,
            start_download,
            pause_download,
            resume_download,
            cancel_download,
            reorder_queue,
            clear_finished,
            get_queue,
            get_settings,
            save_settings,
            detect_browsers,
            engine_info
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
