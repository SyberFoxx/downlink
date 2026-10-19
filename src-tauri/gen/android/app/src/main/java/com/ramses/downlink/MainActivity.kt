package com.ramses.downlink

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts

class MainActivity : TauriActivity() {
  /**
   * Destino de la copia que se está pidiendo ahora mismo.
   *
   * El mismo selector sirve para la imagen de fondo y para el `cookies.txt`, y la
   * respuesta llega por el mismo sitio, así que hay que recordar qué se pidió
   * para copiar el archivo a la carpeta correcta.
   */
  private var pendingPick: PickRequest? = null

  private data class PickRequest(val folder: String, val mimes: Array<String>)

  /**
   * Selector de archivos con copia a la carpeta privada.
   *
   * Android no devuelve una ruta de archivo sino un `content://`, y el motor de
   * descarga (y la lectura del fondo) necesitan una ruta real. Además, el permiso
   * de lectura sobre el URI es temporal: guardarlo tal cual haría que el archivo
   * se perdiera al reabrir la app. Por eso se copia y se devuelve la ruta.
   */
  private val filePicker = registerForActivityResult(
    ActivityResultContracts.OpenDocument(),
  ) { uri -> deliverPickedFile(uri) }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    deliverNativeLibraryDir()
    deliverSharedText(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    deliverSharedText(intent)
  }

  /**
   * Entrega a Rust la carpeta donde Android ha extraído las bibliotecas nativas
   * (`lib/arm64-v8a/`). Ahí viajan los motores (ffmpeg, CPython, quickjs) y es el
   * único directorio desde el que Android deja *ejecutar* algo a una app normal.
   *
   * La ruta la calcula Android y cambia con cada instalación, así que Rust no
   * puede adivinarla: se la tiene que decir esta actividad, y cuanto antes mejor,
   * porque la primera descarga no puede llegar antes que esta llamada.
   */
  private fun deliverNativeLibraryDir() {
    val dir = applicationInfo.nativeLibraryDir ?: return
    if (dir.isEmpty()) return
    pushNativeLibraryDir(dir)
  }

  /**
   * Entrega a Rust el texto que ha compartido otra app. Android lo manda en
   * `EXTRA_TEXT`, muchas veces con el título del vídeo delante; quedarse con el
   * enlace es cosa de la interfaz.
   *
   * `onCreate` cubre el arranque en frío (lo normal al compartir) y `onNewIntent`
   * el caso de que Downlink ya estuviera abierto.
   */
  private fun deliverSharedText(intent: Intent?) {
    if (intent?.action != Intent.ACTION_SEND) return
    // Casi todas las apps lo mandan en `EXTRA_TEXT`; unas pocas, por `ClipData`.
    val text = intent.getStringExtra(Intent.EXTRA_TEXT)
      ?: intent.clipData?.getItemAt(0)?.coerceToText(this)?.toString()
      ?: return
    if (text.isBlank()) return
    pushSharedText(text)
  }

  /**
   * Publica en la galería el archivo que acaba de descargar Rust.
   *
   * La llama Rust por JNI cuando una descarga termina. Devuelve el `content://`
   * de `MediaStore` o `null` si no se pudo; en ese caso el archivo privado sigue
   * donde estaba y la interfaz avisa de que hay que buscarlo a mano.
   */
  @Suppress("unused")
  fun publishToMediaStore(path: String, mimeType: String): String? =
    MediaStoreBridge.publish(this, path, mimeType)

  /**
   * Abre un archivo descargado (ruta real o `content://`). Devuelve el URI que
   * se abrió, o `null` si no hay ninguna app que pueda abrirlo.
   *
   * Devuelve una cadena y no un `boolean` porque Rust llama a este método con un
   * único puente JNI compartido que espera `String`; así los dos métodos —abrir y
   * publicar— usan la misma firma y el código de Rust no se duplica.
   */
  @Suppress("unused")
  fun openDownloaded(pathOrUri: String): String? =
    if (MediaStoreBridge.open(this, pathOrUri)) pathOrUri else null

  /**
   * Abre el selector de imágenes del sistema. Lo llama Rust cuando el usuario
   * pulsa «Elegir imagen» en Apariencia.
   *
   * No devuelve nada: el resultado llega después, por
   * [pushPickedWallpaper]. La interfaz de Rust espera ese dato con un tiempo
   * máximo, así que no puede devolverse aquí.
   */
  @Suppress("unused")
  fun pickWallpaper() {
    // `image/*` sin tipos concretos: cada teléfono expone sus propios MIME y
    // enumerarlos dejaría fuera formatos válidos.
    pickInto("wallpapers", arrayOf("image/*"))
  }

  /**
   * Abre el selector de archivos para el `cookies.txt`. Lo llama Rust.
   *
   * Se aceptan todos los tipos MIME, porque algunos gestores de archivos no marcan
   * el `.txt` como `text/plain` y entonces no dejarían elegirlo.
   */
  @Suppress("unused")
  fun pickCookiesFile() {
    pickInto("cookies", arrayOf("text/plain", "*/*"))
  }

  private fun pickInto(folder: String, mimes: Array<String>) {
    pendingPick = PickRequest(folder, mimes)
    filePicker.launch(mimes)
  }

  /** Copia el archivo elegido y entrega su ruta a Rust (o cadena vacía si no hay). */
  private fun deliverPickedFile(uri: android.net.Uri?) {
    val request = pendingPick
    pendingPick = null
    if (uri == null || request == null) {
      notifyBothPickersEmpty()
      return
    }
    val path = MediaStoreBridge.copyToPrivateStorage(this, uri, request.folder)
    val value = path ?: ""
    // Los dos usos comparten el mismo selector, así que se avisa a los dos; Rust
    // solo está esperando uno y el otro recibe el valor y lo descarta.
    pushPickedWallpaper(value)
    pushPickedCookies(value)
  }

  private fun notifyBothPickersEmpty() {
    pushPickedWallpaper("")
    pushPickedCookies("")
  }

  private external fun pushSharedText(text: String)
  private external fun pushNativeLibraryDir(dir: String)
  private external fun pushPickedWallpaper(path: String)
  private external fun pushPickedCookies(path: String)
}
