//! Puente con `MediaStore` de Android para los archivos descargados.
//!
//! El problema que resuelve: en Android la app solo puede escribir sin permisos
//! dentro de `Android/data/<paquete>/files/`, una carpeta que el usuario **no ve**
//! —ni en la galería ni en los exploradores—. El motor de descarga necesita una
//! carpeta real (`current_dir`), así que el archivo nace ahí y, al terminar, se
//! publica en la biblioteca multimedia del sistema.
//!
//! Publicar crea una entrada en `MediaStore` con su propio `content://`; el vídeo
//! aparece entonces en la galería y en Descargas, y la copia privada se borra
//! para no duplicar el espacio. Todo eso lo hace Kotlin (`MediaStoreBridge`):
//! aquí solo se le pasa la ruta y se espera el `content://` resultante.

#[cfg(target_os = "android")]
use std::sync::mpsc;
#[cfg(target_os = "android")]
use std::time::Duration as StdDuration;

#[cfg(target_os = "android")]
use jni::objects::{JObject, JString, JValue};
#[cfg(target_os = "android")]
use tauri::Manager;

#[cfg(target_os = "android")]
use crate::APP_HANDLE;

/// Tiempo máximo esperando a que el hilo principal conteste. Copiar un vídeo
/// grande a `MediaStore` tarda, y el exceso se deja con margen de sobra: si
/// vence, la publicación se da por perdida pero la descarga sigue siendo válida.
#[cfg(target_os = "android")]
const JNI_TIMEOUT: StdDuration = StdDuration::from_secs(120);

/// Espera máxima a que el usuario elija una imagen. Es más larga que las demás
/// porque depende de una persona, no de la máquina.
#[cfg(target_os = "android")]
const WALLPAPER_TIMEOUT: StdDuration = StdDuration::from_secs(300);

/// Tipo MIME deducido de la extensión, que es lo que `MediaStore` usa para
/// decidir en qué colección entra el archivo (vídeo, audio o descargas).
///
/// Solo importa distinguir vídeo de audio para que el vídeo salga en la galería;
/// lo demás cae en Descargas, que es un sitio razonable para cualquier cosa.
///
/// Fuera de Android nadie la llama (allí los archivos ya están en carpetas
/// visibles), pero se deja compilada para que sus pruebas corran en cualquier
/// máquina: es lógica pura y un error aquí solo se notaría en el teléfono.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn mime_type_for(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "flv" => "video/x-flv",
        "3gp" => "video/3gpp",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "opus" | "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "wav" => "audio/x-wav",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        // Subtítulos y demás acompañantes: texto plano, van a Descargas.
        _ => "application/octet-stream",
    }
}

/// Publica `path` en la biblioteca multimedia y devuelve su `content://`.
///
/// Devuelve `None` si no se pudo (por ejemplo, sin espacio o si Android rechaza
/// la inserción). Quien llama debe tratar ese caso como «publicación no hecha»,
/// no como «descarga fallida»: el archivo sigue existiendo en su carpeta.
#[cfg(target_os = "android")]
pub fn publish(path: &str) -> Option<String> {
    call_string_method("publishToMediaStore", path, mime_type_for(path))
}

/// Abre un archivo descargado, ya sea una ruta real o un `content://`.
///
/// Existe porque el plugin `opener` de Tauri no sabe con `content://`: valida la
/// ruta con `metadata()` y un URI de contenido no es una ruta del sistema.
#[cfg(target_os = "android")]
pub fn open(path_or_uri: &str) -> bool {
    call_string_method("openDownloaded", path_or_uri, "").is_some()
}

/// Resultado pendiente de un selector que contesta más tarde.
///
/// Un selector del sistema no devuelve nada al lanzarlo: la respuesta llega por
/// `onActivityResult`, y de ahí entra por JNI. Este canal conecta las dos mitades.
#[cfg(target_os = "android")]
static WALLPAPER_PICK: std::sync::Mutex<Option<mpsc::Sender<String>>> =
    std::sync::Mutex::new(None);
#[cfg(target_os = "android")]
static COOKIES_PICK: std::sync::Mutex<Option<mpsc::Sender<String>>> =
    std::sync::Mutex::new(None);

/// Abre el selector de imágenes del sistema y espera a que el usuario elija.
///
/// Devuelve la ruta de la copia privada de la imagen, o `None` si se canceló o
/// se pasó del tiempo de espera. La copia la hace Kotlin (el URI del selector no
/// se puede leer más adelante), así que aquí solo se recoge el resultado.
#[cfg(target_os = "android")]
pub fn pick_wallpaper() -> Option<String> {
    pick_into(&WALLPAPER_PICK, "pickWallpaper")
}

/// Abre el selector de archivos y espera al `cookies.txt`.
///
/// yt-dlp necesita una ruta de archivo para `--cookies`, así que se devuelve la
/// de la copia privada que hace Kotlin.
#[cfg(target_os = "android")]
pub fn pick_cookies_file() -> Option<String> {
    pick_into(&COOKIES_PICK, "pickCookiesFile")
}

/// Lanza `method` en la actividad y espera el resultado por `slot`.
#[cfg(target_os = "android")]
fn pick_into(
    slot: &std::sync::Mutex<Option<mpsc::Sender<String>>>,
    method: &str,
) -> Option<String> {
    let (tx, rx) = mpsc::channel::<String>();
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(tx);
    } else {
        return None;
    }

    if !call_void_method(method) {
        if let Ok(mut guard) = slot.lock() {
            *guard = None;
        }
        return None;
    }

    // El usuario puede tardar en elegir, así que aquí se espera más que en el
    // resto de llamadas. Si tarda más, se devuelve «nada elegido» y ya está: no
    // se pierde nada, solo hay que volver a pulsar.
    let picked = rx.recv_timeout(WALLPAPER_TIMEOUT).ok();
    if let Ok(mut guard) = slot.lock() {
        *guard = None;
    }
    picked.filter(|path| !path.is_empty())
}

/// Entrega a Rust el resultado del selector de imagen (cadena vacía = canceló).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ramses_downlink_MainActivity_pushPickedWallpaper(
    mut env: jni::JNIEnv<'_>,
    _this: JObject<'_>,
    path: JString<'_>,
) {
    deliver_pick(&mut env, &path, &WALLPAPER_PICK);
}

/// Entrega a Rust el resultado del selector de cookies (cadena vacía = canceló).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ramses_downlink_MainActivity_pushPickedCookies(
    mut env: jni::JNIEnv<'_>,
    _this: JObject<'_>,
    path: JString<'_>,
) {
    deliver_pick(&mut env, &path, &COOKIES_PICK);
}

/// Guarda en `slot` la ruta que llega por JNI.
///
/// Como ambos selectores comparten un único canal en Kotlin, cada llamada puede
/// recibir el valor del otro; solo el que tiene un receptor esperando lo usa.
#[cfg(target_os = "android")]
fn deliver_pick(
    env: &mut jni::JNIEnv<'_>,
    path: &JString<'_>,
    slot: &std::sync::Mutex<Option<mpsc::Sender<String>>>,
) {
    let Ok(path) = env.get_string(path) else {
        return;
    };
    let Ok(path) = path.to_str() else {
        return;
    };
    let Ok(guard) = slot.lock() else {
        return;
    };
    if let Some(tx) = guard.as_ref() {
        let _ = tx.send(path.to_string());
    }
}

/// Llama a un método de `MainActivity` que no recibe argumentos ni devuelve nada.
#[cfg(target_os = "android")]
fn call_void_method(method: &str) -> bool {
    let Some(app) = APP_HANDLE.get() else {
        return false;
    };
    let Some(window) = app.get_webview_window("main") else {
        return false;
    };
    let (tx, rx) = mpsc::channel::<bool>();
    let method = method.to_string();

    let dispatched = window
        .with_webview(move |webview| {
            webview.jni_handle().exec(move |env, activity, _webview| {
                let ok = env
                    .call_method(activity, &method, "()V", &[])
                    .is_ok();
                let _ = tx.send(ok);
            });
        })
        .is_ok();

    dispatched && rx.recv_timeout(JNI_TIMEOUT).unwrap_or(false)
}

/// Llama a un método de `MainActivity` que recibe dos cadenas y devuelve una
/// `String` (o `null`), esperando el resultado en el hilo principal.
///
/// El método se ejecuta en el hilo de la webview —el único donde Android deja
/// tocar `ContentResolver` con garantías—, así que el resultado vuelve por un
/// canal. Se usa el segundo argumento como «vacío» cuando el método solo espera
/// uno, para no duplicar aquí el puente.
#[cfg(target_os = "android")]
fn call_string_method(method: &str, first: &str, second: &str) -> Option<String> {
    let app = APP_HANDLE.get()?;
    let window = app.get_webview_window("main")?;
    let (tx, rx) = mpsc::channel::<Option<String>>();

    let method = method.to_string();
    let first = first.to_string();
    let second = second.to_string();

    // `jni_handle` solo se alcanza desde la webview de plataforma, que se obtiene
    // en `with_webview`. Ese cierre corre en el hilo principal —el único donde
    // Android deja usar `ContentResolver` con garantías—, así que el resultado
    // vuelve por un canal y aquí se espera.
    window
        .with_webview(move |webview| {
            let jni = webview.jni_handle();
            jni.exec(move |env, activity, _webview| {
                let result = (|| -> Option<String> {
                    let first = env.new_string(&first).ok()?;
                    let second = env.new_string(&second).ok()?;
                    let value = env
                        .call_method(
                            activity,
                            &method,
                            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
                            &[
                                JValue::Object(&JObject::from(first)),
                                JValue::Object(&JObject::from(second)),
                            ],
                        )
                        .ok()?;
                    let value = value.l().ok()?;
                    if value.is_null() {
                        return None;
                    }
                    let text: JString = value.into();
                    let text = env.get_string(&text).ok()?;
                    text.to_str().ok().map(str::to_string)
                })();
                // Si el receptor ya no escucha, el envío falla y no pasa nada.
                let _ = tx.send(result);
            });
        })
        .ok()?;

    rx.recv_timeout(JNI_TIMEOUT).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::mime_type_for;

    #[test]
    fn el_video_y_el_audio_se_distinguen() {
        assert_eq!(mime_type_for("/tmp/a.mp4"), "video/mp4");
        assert_eq!(mime_type_for("/tmp/a.MKV"), "video/x-matroska");
        assert_eq!(mime_type_for("/tmp/a.mp3"), "audio/mpeg");
    }

    #[test]
    fn lo_desconocido_cae_en_descargas() {
        assert_eq!(mime_type_for("/tmp/a.srt"), "application/octet-stream");
        assert_eq!(mime_type_for("/tmp/sin_extension"), "application/octet-stream");
    }
}
