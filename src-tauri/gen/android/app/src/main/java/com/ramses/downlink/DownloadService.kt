package com.ramses.downlink

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat

/**
 * Servicio en primer plano que mantiene viva la app mientras descarga.
 *
 * Android no deja que un proceso en segundo plano siga trabajando: en cuanto la
 * app pierde el foco (o se apaga la pantalla), el sistema puede congelarla o
 * matarla y la descarga se quedaría a medias. Un *foreground service* le dice a
 * Android «esto está haciendo algo que el usuario quiere», y a cambio exige una
 * **notificación permanente** que lo anuncie. Esa notificación es también la
 * única señal honesta de que hay algo en marcha, así que muestra el progreso.
 *
 * No hace la descarga: yt-dlp vive en el proceso nativo, que es el mismo de la
 * app. El servicio sirve para que ese proceso **no muera** y para llevar la
 * cuenta en la barra de estado.
 */
class DownloadService : Service() {
    companion object {
        private const val CHANNEL_ID = "downlink_downloads"
        private const val NOTIFICATION_ID = 1
        /** Arranca el servicio con el texto inicial. */
        const val ACTION_START = "com.ramses.downlink.service.START"
        const val ACTION_UPDATE = "com.ramses.downlink.service.UPDATE"
        const val ACTION_STOP = "com.ramses.downlink.service.STOP"
        const val EXTRA_TEXT = "text"
        const val EXTRA_PERCENT = "percent"

        /** Arranca o actualiza el servicio; `text` es lo que se lee en la notificación. */
        fun start(context: Context, text: String) {
            val intent = Intent(context, DownloadService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_TEXT, text)
            }
            // `startForegroundService` es obligatorio desde Android 8: el sistema
            // da unos segundos para llamar a `startForeground` o mata el servicio.
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /** Refresca el progreso (texto y porcentaje; `percent` < 0 = indeterminado). */
        fun update(context: Context, text: String, percent: Int) {
            val intent = Intent(context, DownloadService::class.java).apply {
                action = ACTION_UPDATE
                putExtra(EXTRA_TEXT, text)
                putExtra(EXTRA_PERCENT, percent)
            }
            context.startService(intent)
        }

        /** Retira la notificación y deja que el sistema reclame el proceso. */
        fun stop(context: Context) {
            context.stopService(Intent(context, DownloadService::class.java))
        }
    }

    private var lastPercent = -1

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            // `stopService` no llega aquí (el sistema para el servicio directamente),
            // pero por si acaso, atender el intent de parada.
            ACTION_STOP -> {
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_UPDATE -> {
                val text = intent.getStringExtra(EXTRA_TEXT).orEmpty()
                val percent = intent.getIntExtra(EXTRA_PERCENT, -1)
                notify(build(text, percent))
                return START_NOT_STICKY
            }
            else -> {
                val text = intent?.getStringExtra(EXTRA_TEXT).orEmpty()
                // En el primer arranque hay que entrar en primer plano **ya**.
                startForeground(NOTIFICATION_ID, build(text, -1))
                return START_NOT_STICKY
            }
        }
    }

    /** La notificación que se queda fija mientras el servicio vive. */
    private fun build(text: String, percent: Int): Notification {
        ensureChannel()
        // Tocar la notificación vuelve a la app sin recrear la actividad.
        val open = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java).apply {
                flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
            },
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        val builder = NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("Downlink")
            .setContentText(text.ifBlank { "Descargando…" })
            .setSmallIcon(android.R.drawable.stat_sys_download)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(open)
            .setPriority(NotificationCompat.PRIORITY_LOW)

        if (percent in 0..100) {
            builder.setProgress(100, percent, false)
        } else {
            builder.setProgress(0, 0, true)
        }
        // Cuántas descargas van: un número pequeño a la derecha.
        builder.setSubText("En curso")
        return builder.build()
    }

    private fun notify(notification: Notification) {
        val manager = getSystemService(NotificationManager::class.java) ?: return
        manager.notify(NOTIFICATION_ID, notification)
    }

    /** Crea el canal una sola vez; en Android 8+ sin canal no hay notificación. */
    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java) ?: return
        if (manager.getNotificationChannel(CHANNEL_ID) != null) return
        val channel = NotificationChannel(
            CHANNEL_ID,
            "Descargas en curso",
            // Importancia mínima: es una notificación informativa que no debe
            // sonar ni vibrar cada vez que cambia el porcentaje.
            NotificationManager.IMPORTANCE_LOW,
        ).apply {
            description = "Muestra la descarga que está en marcha."
            setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
    }
}
