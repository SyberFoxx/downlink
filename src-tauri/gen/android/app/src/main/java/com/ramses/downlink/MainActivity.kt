package com.ramses.downlink

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    deliverSharedText(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    deliverSharedText(intent)
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

  private external fun pushSharedText(text: String)
}
