mod engine;
mod history;
mod media_store;
mod queue;
mod queue_store;
mod settings;
#[cfg(target_os = "android")]
mod share;
mod theme;
#[cfg(desktop)]
mod tray;
mod updater;
mod ytdlp;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tauri::{Manager, State};

#[cfg(target_os = "android")]
use jni::JNIEnv;
#[cfg(target_os = "android")]
use jni::objects::{JObject, JString};

use history::HistoryState;
use queue::{JobView, Queue};
use settings::{Settings, SettingsState};
use ytdlp::DownloadOptions;

/// Estado global de la cola de descargas.
struct QueueState(Arc<Queue>);

/// Enlace que ha compartido otra app (Android) y que la interfaz todavía no ha
/// recogido. El caso frío es el normal: el sistema lanza la app al compartir, así
/// que el texto llega mucho antes de que exista la webview. Se guarda aquí y la
/// interfaz lo pide al arrancar con `take_shared_text`; si ya está en marcha, el
/// lado Android además emite `share-intent`.
#[derive(Default)]
pub struct SharedLink(Mutex<Option<String>>);

impl SharedLink {
    /// Guarda el enlace pendiente. Devuelve `true` si hay algo que anunciar.
    /// Solo se conserva el último: interesa lo que el usuario acaba de compartir.
    pub fn push(&self, text: &str) -> bool {
        let trimmed = text.trim();
        let Ok(mut slot) = self.0.lock() else {
            return false;
        };
        if trimmed.is_empty() {
            return false;
        }
        *slot = Some(trimmed.to_string());
        true
    }

    /// Entrega el enlace pendiente y lo limpia: se trata una sola vez.
    pub fn take(&self) -> Option<String> {
        self.0.lock().ok().and_then(|mut slot| slot.take())
    }
}

/// Handle de la app para la capa Java: el texto compartido entra por una función
/// JNI, que no recibe `State`, así que hay que tenerlo a mano para guardarlo y
/// avisar a la interfaz.
#[cfg(target_os = "android")]
pub(crate) static APP_HANDLE: std::sync::OnceLock<tauri::AppHandle> = std::sync::OnceLock::new();

/// Llama `MainActivity` para entregar la carpeta de bibliotecas nativas
/// (`applicationInfo.nativeLibraryDir`), que es donde el instalador extrae los
/// motores y el único sitio desde el que Android deja ejecutarlos.
///
/// Android recalcula esa ruta en cada instalación, así que la única forma fiable
/// de conocerla es preguntársela. El lado Kotlin la manda en cuanto arranca la
/// actividad, antes de que la interfaz pueda pedir una descarga.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ramses_downlink_MainActivity_pushNativeLibraryDir(
    mut env: JNIEnv<'_>,
    _this: JObject<'_>,
    dir: JString<'_>,
) {
    let Ok(dir) = env.get_string(&dir) else {
        return;
    };
    let Ok(dir) = dir.to_str() else {
        return;
    };
    engine::set_bundled_lib_dir(dir.trim());
}

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
/// Recoge (y limpia) el enlace que compartió otra app antes de que la interfaz
/// cargara. Devuelve `None` si no hay nada pendiente.
#[tauri::command]
fn take_shared_text(state: State<'_, SharedLink>) -> Option<String> {
    state.take()
}

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

/// Comprueba si conviene descargar el runtime JS gestionado (quickjs).
#[tauri::command]
async fn check_js_runtime_update() -> Result<updater::UpdateCheck, String> {
    updater::check_js_runtime().await
}

/// Descarga e instala el runtime JS (quickjs). El progreso llega por el evento
/// `engine-update`.
#[tauri::command]
async fn update_js_runtime(app: tauri::AppHandle) -> Result<updater::UpdateCheck, String> {
    updater::update_js_runtime(&app).await
}

/// Último progreso de actualización de motor visto en esta sesión.
///
/// Existe para la carrera de arranque: `auto_update` se lanza antes de que la
/// UI haya podido suscribirse al evento `engine-update`, y sin este estado el
/// aviso no aparecería cuando la app se descarga los motores sola al abrirse.
#[tauri::command]
fn engine_update_state() -> Option<updater::UpdateProgress> {
    updater::last_progress()
}

/// Versión de la aplicación, para mostrarla sin repetirla a mano en la UI.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
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

/// Carpetas propias de la app donde se puede escribir sin permisos (Android).
///
/// Existe porque el selector de carpetas del sistema **no sirve** en Android: no
/// devuelve una ruta, sino un `content://…`, y el motor de descarga necesita una
/// carpeta real como `current_dir`. Estas son las que Android deja usar sin pedir
/// permisos: viven bajo `Android/data/<paquete>/files/`.
///
/// En el escritorio no se usa (allí el selector del sistema da rutas de verdad),
/// pero se devuelven las del sistema para no dejar el comando sin respuesta.
#[tauri::command]
fn app_folders(app: tauri::AppHandle) -> Vec<String> {
    let mut folders: Vec<PathBuf> = Vec::new();
    let path = app.path();
    // `download_dir` es la que Tauri resuelve por defecto y la que se usaba hasta
    // ahora; las demás dejan organizar por tipo sin salirse de lo escribible.
    for candidate in [path.download_dir(), path.video_dir(), path.audio_dir()] {
        if let Ok(dir) = candidate
            && !folders.contains(&dir)
        {
            folders.push(dir);
        }
    }
    folders
        .into_iter()
        .map(|dir| dir.display().to_string())
        .collect()
}

/// Abre el selector de imágenes del sistema y devuelve la ruta de la copia
/// privada de la imagen elegida (Android).
///
/// El selector de archivos de Tauri no sirve aquí por partida doble: en Android
/// devuelve un `content://` y el permiso de lectura sobre él es temporal, así que
/// el fondo se perdería al reabrir la app. El lado nativo copia la imagen y
/// devuelve una ruta real.
#[tauri::command]
async fn pick_wallpaper() -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(media_store::pick_wallpaper)
            .await
            .map_err(|e| format!("No se pudo abrir el selector de imágenes: {e}"))
    }
    #[cfg(not(target_os = "android"))]
    {
        Err("Elegir imagen así solo se usa en Android.".to_string())
    }
}

/// Abre el selector de archivos y devuelve la ruta de la copia privada del
/// `cookies.txt` elegido (Android).
///
/// yt-dlp necesita una ruta de archivo para `--cookies`, y el selector de Android
/// solo da un `content://`: sin copiarlo, las cookies nunca llegarían al motor.
/// Eso explica que «el problema de las cookies» siguiera igual aunque el archivo
/// se eligiera bien.
#[tauri::command]
async fn pick_cookies_file() -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        tauri::async_runtime::spawn_blocking(media_store::pick_cookies_file)
            .await
            .map_err(|e| format!("No se pudo abrir el selector de archivos: {e}"))
    }
    #[cfg(not(target_os = "android"))]
    {
        Err("Elegir archivos así solo se usa en Android.".to_string())
    }
}

/// Indica si la bandeja del sistema está disponible (para poder desactivar el
/// ajuste «cerrar a la bandeja» en la interfaz cuando no lo está).
#[tauri::command]
fn tray_available(app: tauri::AppHandle) -> bool {
    app.try_state::<RuntimeFlags>()
        .map(|flags| flags.tray_available.load(Ordering::Relaxed))
        .unwrap_or(false)
}

/// Abre un archivo descargado con la app que corresponda (Android).
///
/// En el escritorio el plugin `opener` ya lo hace bien, así que este comando solo
/// existe para Android: allí un archivo publicado en la galería se identifica con
/// un `content://`, y `open_path` de Tauri lo rechaza porque comprueba la ruta
/// con `metadata()`. Aquí se delega en `MediaStoreBridge`, que lanza el
/// `ACTION_VIEW` a mano tanto para rutas reales como para URIs de contenido.
///
/// Es `async` y el puente se ejecuta en un hilo aparte a propósito: abrir exige
/// hablar con el hilo principal, y si se hiciera desde el propio hilo principal
/// (como correría un comando síncrono) este se quedaría esperando su propia
/// respuesta y no habría forma de abrir nada.
#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        let abierto = tauri::async_runtime::spawn_blocking(move || media_store::open(&path))
            .await
            .unwrap_or(false);
        if abierto {
            return Ok(());
        }
        Err("No hay ninguna aplicación que pueda abrir este archivo (o ya no está).".to_string())
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = path;
        Err("Abrir archivos así solo se usa en Android.".to_string())
    }
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
            app.manage(SharedLink::default());

            // Deja el handle a mano para el lado Java (ver `share.rs`).
            #[cfg(target_os = "android")]
            let _ = APP_HANDLE.set(app.handle().clone());

            // Bandeja del sistema (solo escritorio; en móvil no existe).
            #[cfg(desktop)]
            let tray_available_flag = match tray::setup(app.handle()) {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("No se pudo inicializar la bandeja: {error}");
                    false
                }
            };
            #[cfg(not(desktop))]
            let tray_available_flag = false;
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
        .on_window_event(|_window, _event| {
            #[cfg(desktop)]
            if let tauri::WindowEvent::CloseRequested { api, .. } = _event
                && let Some(flags) = _window.try_state::<RuntimeFlags>()
            {
                let close_to_tray = flags.close_to_tray.load(Ordering::Relaxed);
                let tray_available = flags.tray_available.load(Ordering::Relaxed);
                if close_to_tray && tray_available && _window.label() == "main" {
                    api.prevent_close();
                    let _ = _window.hide();
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
            take_shared_text,
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
            check_js_runtime_update,
            update_js_runtime,
            engine_update_state,
            app_version,
            read_wallpaper,
            read_system_palette,
            default_download_dir,
            app_folders,
            open_file,
            pick_wallpaper,
            pick_cookies_file,
            tray_available
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::SharedLink;

    /// El enlace compartido se entrega exactamente una vez: si la interfaz lo
    /// pidiera dos veces, se analizaría dos veces el mismo enlace.
    #[test]
    fn el_enlace_compartido_se_entrega_una_sola_vez() {
        let link = SharedLink::default();
        assert_eq!(link.take(), None);

        assert!(link.push("  https://youtu.be/abc  "));
        assert_eq!(link.take().as_deref(), Some("https://youtu.be/abc"));
        assert_eq!(link.take(), None);

        // El último compartido es el que vale, y un texto vacío no pisa nada.
        assert!(link.push("https://youtu.be/uno"));
        assert!(link.push("https://youtu.be/dos"));
        assert!(!link.push("   "));
        assert_eq!(link.take().as_deref(), Some("https://youtu.be/dos"));
    }

    /// La versión que muestra la interfaz sale del `Cargo.toml`; los
    /// instaladores, del `tauri.conf.json`. Si se bumpa uno y no el otro, la
    /// app publicada diría una versión distinta de la que anuncia su release.
    #[test]
    fn la_version_coincide_con_tauri_conf() {
        let conf = include_str!("../tauri.conf.json");
        let esperado = format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION"));
        assert!(
            conf.contains(&esperado),
            "tauri.conf.json no declara {esperado}; revisa el bump de versión"
        );
    }
}
