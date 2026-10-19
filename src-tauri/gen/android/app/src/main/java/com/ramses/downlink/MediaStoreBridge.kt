package com.ramses.downlink

import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File

/**
 * Publica un archivo descargado en la biblioteca multimedia de Android.
 *
 * Sin esto, el vídeo acaba en `Android/data/com.ramses.downlink/files/Download`,
 * una carpeta que el usuario **no ve**: no aparece en la galería ni en los
 * exploradores de archivos, porque Android 11+ oculta `Android/data`. Ese
 * directorio es el único donde la app puede escribir sin permisos, así que la
 * descarga se hace ahí y, al terminar, se copia a `MediaStore`.
 *
 * Al insertar en `MediaStore` se crea una entrada con su propio `content://`,
 * que es lo que hace que el vídeo salga en la galería y en Descargas. Después se
 * borra la copia privada para no ocupar el doble.
 *
 * Se usa `MediaStore` directamente y no un árbol SAF elegido por el usuario
 * porque el flujo es automático: el usuario no tiene que dar permiso a nada ni
 * elegir carpeta, y el resultado queda en el sitio estándar.
 */
object MediaStoreBridge {
    /** Colección de `MediaStore` que corresponde a un archivo, según su tipo. */
    private fun collectionFor(mimeType: String): Uri =
        when {
            mimeType.startsWith("video/") -> MediaStore.Video.Media.EXTERNAL_CONTENT_URI
            mimeType.startsWith("audio/") -> MediaStore.Audio.Media.EXTERNAL_CONTENT_URI
            mimeType.startsWith("image/") -> MediaStore.Images.Media.EXTERNAL_CONTENT_URI
            else -> MediaStore.Downloads.EXTERNAL_CONTENT_URI
        }

    /** Carpeta dentro de `MediaStore` donde se deja el archivo. */
    private fun relativePathFor(mimeType: String): String =
        when {
            mimeType.startsWith("video/") -> Environment.DIRECTORY_MOVIES
            mimeType.startsWith("audio/") -> Environment.DIRECTORY_MUSIC
            mimeType.startsWith("image/") -> Environment.DIRECTORY_PICTURES
            else -> Environment.DIRECTORY_DOWNLOADS
        } + "/Downlink"

    /**
     * Copia `source` a la biblioteca multimedia y devuelve el `content://` de la
     * entrada creada, o `null` si no se pudo. En caso de fallo el archivo privado
     * se deja intacto: más vale que la descarga siga existiendo aunque sea en la
     * carpeta oculta que perderla.
     */
    fun publish(context: Context, source: String, mimeType: String): String? {
        val file = File(source)
        if (!file.isFile) return null

        // Android 9 y anteriores no tienen `MediaStore.Downloads`: se copia al
        // almacenamiento compartido a pelo, que allí sí se puede sin permisos
        // especiales para las carpetas públicas de la app.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            return publishLegacy(context, file, mimeType)
        }

        val collection = collectionFor(mimeType)
        val values = ContentValues().apply {
            put(MediaStore.MediaColumns.DISPLAY_NAME, file.name)
            put(MediaStore.MediaColumns.MIME_TYPE, mimeType)
            put(MediaStore.MediaColumns.RELATIVE_PATH, relativePathFor(mimeType))
            put(MediaStore.MediaColumns.IS_PENDING, 1)
        }

        val resolver = context.contentResolver
        val target = try {
            resolver.insert(collection, values)
        } catch (_: Exception) {
            null
        } ?: return null

        val copied = try {
            resolver.openOutputStream(target)?.use { output ->
                file.inputStream().use { input -> input.copyTo(output) }
            }
            true
        } catch (_: Exception) {
            false
        }

        if (!copied) {
            // Entrada a medias: se retira para no dejar un fantasma en la galería.
            try {
                resolver.delete(target, null, null)
            } catch (_: Exception) {
                // Da igual: el `IS_PENDING` ya la mantiene oculta.
            }
            return null
        }

        values.clear()
        values.put(MediaStore.MediaColumns.IS_PENDING, 0)
        try {
            resolver.update(target, values, null, null)
        } catch (_: Exception) {
            // El archivo ya está completo; la marca solo lo publicaba.
        }

        // La copia privada sobra: el archivo ya vive en la galería.
        file.delete()
        return target.toString()
    }

    /** Copia a la carpeta pública para Android 9 y anteriores. */
    private fun publishLegacy(context: Context, file: File, mimeType: String): String? {
        // `getExternalStoragePublicDirectory` ya devuelve la carpeta del tipo
        // (Movies, Music…); solo falta la subcarpeta propia de la app.
        val typeDir = relativePathFor(mimeType).substringBefore('/')
        val dir = File(
            Environment.getExternalStoragePublicDirectory(typeDir),
            "Downlink",
        )
        if (!dir.isDirectory && !dir.mkdirs()) return null
        val target = File(dir, file.name)
        return try {
            file.inputStream().use { input ->
                target.outputStream().use { output -> input.copyTo(output) }
            }
            file.delete()
            notifyGallery(context, target, mimeType)
            // Se devuelve la ruta real, no un `file://`: es una ruta del sistema
            // (almacenamiento compartido) y así la abren igual el `FileProvider`
            // y cualquier comprobación de existencia.
            target.absolutePath
        } catch (_: Exception) {
            null
        }
    }

    /** Avisa al sistema de que hay un archivo nuevo (solo Android 9 y antes). */
    private fun notifyGallery(context: Context, file: File, mimeType: String) {
        try {
            android.media.MediaScannerConnection.scanFile(
                context,
                arrayOf(file.absolutePath),
                arrayOf(mimeType),
                null,
            )
        } catch (_: Exception) {
            // Es solo un aviso para que la galería lo vea antes; no es crítico.
        }
    }

    /**
     * Copia el contenido de un `content://` a la carpeta privada de la app y
     * devuelve la ruta real, o `null` si no se pudo.
     *
     * Se usa para la imagen de fondo: el selector del sistema entrega un URI con
     * permiso de lectura temporal, así que guardarlo tal cual haría que el fondo
     * se perdiera al reabrir la app. Copiarlo resuelve eso y, de paso, deja una
     * ruta de archivo que el resto del código ya sabe leer.
     *
     * Antes de copiar se vacía la carpeta de destino: el fondo anterior ya no
     * sirve y acumular copias dejaría basura invisible en el almacenamiento.
     */
    fun copyToPrivateStorage(context: Context, uri: Uri, folder: String): String? {
        val dir = File(context.filesDir, folder)
        if (dir.isDirectory) {
            dir.listFiles()?.forEach { it.delete() }
        } else if (!dir.mkdirs()) {
            return null
        }

        val name = queryDisplayName(context, uri) ?: "wallpaper"
        // El nombre viene de fuera: se recorta a la última ruta y se le quita
        // cualquier separador para que no pueda escapar de la carpeta.
        val safeName = name.substringAfterLast('/').substringAfterLast('\\')
            .replace("..", "").ifBlank { "wallpaper" }
        val target = File(dir, safeName)

        return try {
            context.contentResolver.openInputStream(uri)?.use { input ->
                target.outputStream().use { output -> input.copyTo(output) }
            } ?: return null
            target.absolutePath
        } catch (_: Exception) {
            target.delete()
            null
        }
    }

    /** Nombre que el proveedor da al archivo elegido, si lo tiene. */
    private fun queryDisplayName(context: Context, uri: Uri): String? = try {
        context.contentResolver.query(
            uri,
            arrayOf(android.provider.OpenableColumns.DISPLAY_NAME),
            null,
            null,
            null,
        )?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        }
    } catch (_: Exception) {
        null
    }

    /**
     * Abre un archivo ya sea una ruta real o un `content://` de `MediaStore`.
     *
     * El plugin `opener` de Tauri no sirve para lo segundo: comprueba que la ruta
     * exista con `metadata()`, y un `content://` no es una ruta del sistema. Aquí
     * se construye el `ACTION_VIEW` a mano, que es lo que entiende Android.
     */
    fun open(context: Context, pathOrUri: String): Boolean {
        return try {
            val uri = if (pathOrUri.startsWith("content://")) {
                Uri.parse(pathOrUri)
            } else {
                val file = File(pathOrUri)
                if (!file.isFile) return false
                FileProvider.getUriForFile(
                    context,
                    "${context.packageName}.fileprovider",
                    file,
                )
            }
            val mime = context.contentResolver.getType(uri) ?: "*/*"
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, mime)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
            context.startActivity(intent)
            true
        } catch (_: Exception) {
            false
        }
    }
}
