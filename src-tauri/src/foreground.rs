//! Servicio en primer plano de Android: mantiene viva la descarga y avisa en la
//! barra de estado.
//!
//! Android no deja que un proceso en segundo plano siga trabajando: al perder el
//! foco (o al apagarse la pantalla) el sistema puede congelarlo o matarlo y la
//! descarga se quedaría a medias. Un *foreground service* evita eso, y a cambio
//! exige una notificación permanente. La lógica vive en Kotlin
//! (`DownloadService`): aquí solo se la arranca, se refresca y se para.
//!
//! Todas las llamadas son **de ida y vuelta por JNI**, pero no se espera
//! respuesta: refrescar el progreso ocurre muchas veces por segundo y bloquear el
//! hilo de la descarga en cada refresco no aportaría nada.

#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};
#[cfg(target_os = "android")]
use tauri::Manager;

#[cfg(target_os = "android")]
use crate::APP_HANDLE;

/// ¿Está el servicio ya en marcha? En Android, la primera llamada tiene que ser
/// `startForegroundService` (y llegar a `startForeground` a tiempo, o el sistema
/// mata el servicio); las siguientes van con `startService`. Este indicador evita
/// repetir la primera.
#[cfg(target_os = "android")]
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Arranca el servicio con un texto inicial. Solo tiene efecto en Android.
#[cfg(target_os = "android")]
pub fn start(text: &str) {
    call_simple("startDownloadService", text);
    RUNNING.store(true, Ordering::Relaxed);
}

/// Refresca la notificación. `percent` va de 0 a 100, o -1 si no se conoce.
///
/// Si el servicio aún no estaba en marcha, lo arranca: así los llamantes no
/// tienen que distinguir el primer aviso de los siguientes.
#[cfg(target_os = "android")]
pub fn update(text: &str, percent: i32) {
    if !RUNNING.load(Ordering::Relaxed) {
        start(text);
    }
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let text = text.to_string();
    let _ = window.with_webview(move |webview| {
        webview.jni_handle().exec(move |env, activity, _webview| {
            let Ok(arg) = env.new_string(&text) else {
                return;
            };
            let _ = env.call_method(
                activity,
                "updateDownloadService",
                "(Ljava/lang/String;I)V",
                &[
                    JValue::Object(&JObject::from(arg)),
                    JValue::Int(percent),
                ],
            );
        });
    });
}

/// Para el servicio y retira la notificación.
#[cfg(target_os = "android")]
pub fn stop() {
    if !RUNNING.swap(false, Ordering::Relaxed) {
        return;
    }
    call_simple("stopDownloadService", "");
}

/// Llama a un método `(String) -> void` de `MainActivity` sin esperar resultado.
#[cfg(target_os = "android")]
fn call_simple(method: &str, text: &str) {
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let method = method.to_string();
    let text = text.to_string();
    let _ = window.with_webview(move |webview| {
        webview.jni_handle().exec(move |env, activity, _webview| {
            let Ok(arg) = env.new_string(&text) else {
                return;
            };
            let _ = env.call_method(
                activity,
                &method,
                "(Ljava/lang/String;)V",
                &[JValue::Object(&JObject::from(arg))],
            );
        });
    });
}
