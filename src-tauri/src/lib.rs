mod engine;
mod history;
mod queue;
mod queue_store;
mod settings;
mod theme;
#[cfg(desktop)]
mod tray;
mod updater;
mod ytdlp;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tauri::WindowEvent;
use tauri::{Manager, State};

use history::HistoryState;
use queue::{JobView, Queue};
use settings::{Settings, SettingsState};
use ytdlp::DownloadOptions;

/// Estado global de la cola de descargas.
struct QueueState(Arc<Queue>);

/// Banderas en tiempo de ejecución para la UX de escritorio.
struct RuntimeFlags {
    close_to_tray: AtomicBool,
    tray_available: AtomicBool,
}

impl RuntimeFlags {
    fn new(close_to_tray: bool, tray_available: bool) -> Self {
        Self {
            close_to_tray: AtomicBool::new(close_to_tray),
            tray_available: AtomicBool::new(tray_available),
        }
    }
}

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
    thumbnail: Option<String>,
    options: DownloadOptions,
) -> Result<u64, String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    let queue = app.state::<QueueState>().0.clone();
    let title = title
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| url.clone());
    Ok(queue
        .enqueue(&settings, url, title, thumbnail, options)
        .await)
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

/// Reintenta un trabajo fallido o cancelado, conservando sus opciones.
#[tauri::command]
async fn retry_download(app: tauri::AppHandle, id: u64) -> Result<(), String> {
    let settings = app.state::<SettingsState>().0.lock().await.clone();
    app.state::<QueueState>().0.retry(&settings, id).await;
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

/// Historial de descargas completadas, de la más reciente a la más antigua.
#[tauri::command]
async fn get_history(app: tauri::AppHandle) -> Result<Vec<history::HistoryEntry>, String> {
    Ok(app.state::<HistoryState>().list().await)
}

/// Quita una descarga del historial (por su URL).
#[tauri::command]
async fn remove_history(app: tauri::AppHandle, url: String) -> Result<(), String> {
    app.state::<HistoryState>().remove(&app, &url).await;
    Ok(())
}

/// Vacía el historial por completo.
#[tauri::command]
async fn clear_history(app: tauri::AppHandle) -> Result<(), String> {
    app.state::<HistoryState>().clear(&app).await;
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
    {
        let settings_clone = settings.clone();
        *app.state::<SettingsState>().0.lock().await = settings_clone;
    }

    if let Some(flags) = app.try_state::<RuntimeFlags>() {
        flags
            .close_to_tray
            .store(settings.close_to_tray, Ordering::Relaxed);
    }

    // Si se desactiva la persistencia, no dejamos una cola antigua en disco.
    if !settings.persist_queue {
        let _ = queue_store::clear(&app);
    }

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

/// Comprueba si hay una versión más reciente de yt-dlp publicada.
#[tauri::command]
async fn check_ytdlp_update() -> Result<updater::UpdateCheck, String> {
    updater::check_ytdlp().await
}

/// Descarga e instala la última versión de yt-dlp. El progreso llega por el
/// evento `engine-update`.
#[tauri::command]
async fn update_ytdlp(app: tauri::AppHandle) -> Result<updater::UpdateCheck, String> {
    updater::update_ytdlp(&app).await
}

/// Comprueba si conviene descargar o actualizar la copia gestionada de ffmpeg.
#[tauri::command]
async fn check_ffmpeg_update() -> Result<updater::UpdateCheck, String> {
    updater::check_ffmpeg().await
}

/// Descarga e instala ffmpeg (y ffprobe). El progreso llega por el evento
/// `engine-update`.
#[tauri::command]
async fn update_ffmpeg(app: tauri::AppHandle) -> Result<updater::UpdateCheck, String> {
    updater::update_ffmpeg(&app).await
}

/// Lee la imagen de fondo y la devuelve como `data:` URL lista para CSS.
#[tauri::command]
async fn read_wallpaper(path: String) -> Result<String, String> {
    theme::read_wallpaper(&path)
}

/// Paleta del escritorio generada por pywal/wallust, si existe.
#[tauri::command]
async fn read_system_palette() -> Option<theme::SystemPalette> {
    theme::read_system_palette()
}

/// Carpeta de descargas por defecto del sistema.
#[tauri::command]
fn default_download_dir(app: tauri::AppHandle) -> Option<String> {
    app.path()
        .download_dir()
        .ok()
        .map(|path| path.display().to_string())
}

/// Indica si la bandeja del sistema está disponible (para poder desactivar el
/// ajuste «cerrar a la bandeja» en la interfaz cuando no lo está).
#[tauri::command]
fn tray_available(app: tauri::AppHandle) -> bool {
    app.try_state::<RuntimeFlags>()
        .map(|flags| flags.tray_available.load(Ordering::Relaxed))
        .unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "linux")]
    apply_webkit_workarounds();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            engine::init(app.handle());
            let settings = settings::load(app.handle());
            let concurrency = settings.concurrency as usize;
            app.manage(SettingsState(tokio::sync::Mutex::new(settings.clone())));
            app.manage(HistoryState(tokio::sync::Mutex::new(history::load(
                app.handle(),
            ))));
            let queue = Arc::new(Queue::new(app.handle().clone(), concurrency));
            app.manage(QueueState(queue.clone()));

            // Bandeja del sistema (desktop).
            let mut tray_available_flag = false;
            #[cfg(desktop)]
            {
                if let Err(error) = tray::setup(app.handle()) {
                    eprintln!("No se pudo inicializar la bandeja: {error}");
                } else {
                    tray_available_flag = true;
                }
            }
            app.manage(RuntimeFlags::new(
                settings.close_to_tray,
                tray_available_flag,
            ));

            // Restaura la cola de la sesión anterior.
            tauri::async_runtime::spawn(async move {
                queue.restore(&settings).await;
            });

            // Actualiza los motores en segundo plano al arrancar (si está activado).
            let handle_updater = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                updater::auto_update(&handle_updater).await;
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            #[cfg(desktop)]
            if let WindowEvent::CloseRequested { api, .. } = event
                && let Some(flags) = window.try_state::<RuntimeFlags>()
            {
                let close_to_tray = flags.close_to_tray.load(Ordering::Relaxed);
                let tray_available = flags.tray_available.load(Ordering::Relaxed);
                if close_to_tray && tray_available && window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            probe_url,
            start_download,
            pause_download,
            resume_download,
            cancel_download,
            retry_download,
            reorder_queue,
            clear_finished,
            get_queue,
            get_history,
            remove_history,
            clear_history,
            get_settings,
            save_settings,
            detect_browsers,
            engine_info,
            check_ytdlp_update,
            update_ytdlp,
            check_ffmpeg_update,
            update_ffmpeg,
            read_wallpaper,
            read_system_palette,
            default_download_dir,
            tray_available
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
