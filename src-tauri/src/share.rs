//! Recepción de enlaces compartidos desde otras apps (Android).
//!
//! `MainActivity` lee el texto del *share intent* y lo entrega aquí por JNI. Es la
//! única vía que no pasa por la webview, que en un arranque en frío todavía no
//! existe: por eso el texto se guarda en `SharedLink` y la interfaz lo recoge al
//! montar, o escucha `share-intent` si ya estaba en marcha.

use crate::{APP_HANDLE, SharedLink};
use jni::JNIEnv;
use jni::objects::{JObject, JString};
use tauri::{Emitter, Manager};

/// La llama `MainActivity` (`external fun pushSharedText`) desde Kotlin.
///
/// El nombre sigue el formato de JNI (`Java_<paquete>_<clase>_<método>`): no se
/// puede cambiar sin cambiar también el lado Kotlin.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_ramses_downlink_MainActivity_pushSharedText(
    mut env: JNIEnv,
    _this: JObject,
    text: JString,
) {
    let Ok(text) = env.get_string(&text) else {
        return;
    };
    let Ok(text) = text.to_str() else {
        return;
    };
    let Some(app) = APP_HANDLE.get() else {
        return;
    };
    if app.state::<SharedLink>().push(text) {
        let _ = app.emit("share-intent", text);
    }
}
