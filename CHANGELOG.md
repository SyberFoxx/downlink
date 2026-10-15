# Changelog

Todo lo notable de Downlink se documenta aquí.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/),
y el proyecto adhiere a [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

## [0.5.0] - 2026-10-15

### Added
- **Historial con galería**: cada descarga completada se guarda (miniatura, título, fecha, tamaño y resumen de opciones) y se ve en la nueva vista Historial
- **Búsqueda y re-descarga**: filtra por título o enlace, vuelve a descargar con un click (reutiliza las opciones originales) o abre el archivo en su carpeta
- **Persistencia local**: el historial se guarda en `history.json` (hasta 500 entradas, sin duplicados por enlace) y sobrevive al reinicio
- Comando `start_download` acepta la miniatura para mostrarla en la galería
- 3 tests nuevos del historial (38 en total)

### Changed
- El sidebar activa la sección **Historial** (antes «próximamente»)

## [0.4.1] - 2026-10-15

### Fixed
- **Las radios/mixes de YouTube (`list=RD…`, `start_radio=1`) colgaban la vista al analizarlas**: devuelven cientos de elementos con vídeos repetidos, y las claves duplicadas rompían el listado de la playlist. Ahora las entradas se **deduplican por id** y el listado usa el índice como clave
- Las radios se **limitan a 50 elementos** (`--playlist-end`): son listas autogeneradas, repiten canciones y pueden ser casi infinitas. Se avisa en la UI de que la lista está recortada

## [0.4.0] - 2026-10-15

### Added
- **Playlists y canales**: al pegar el enlace de una playlist se listan todos sus vídeos con título y duración, se pueden seleccionar (todos/ninguno/uno a uno) y van a la cola de golpe
- **Arrastrar o pegar playlists enteras**: una playlist en el portapapeles (o arrastrada) encola todos sus vídeos
- Resoluciones estándar de reserva cuando el enlace no las reporta (p. ej. dentro de playlists)

### Fixed
- **Barra de progreso sin porcentaje**: `progress.percent` de yt-dlp devuelve `NA` en muchas versiones, así que el porcentaje y el ETA se calculan ahora en Rust a partir de los bytes y la velocidad
- El cálculo usa `total_bytes_estimate` cuando el total real aún no se conoce

## [0.3.0] - 2026-10-15

### Added
- **Cola de descargas real**: varios trabajos en marcha con estados (en cola, descargando, procesando, en pausado, completado, error, cancelado)
- **Concurrencia configurable** (1–8 descargas simultáneas) desde Ajustes, aplicada al instante
- **Pausar y reanudar** una descarga en curso sin empezar de cero (yt-dlp continúa el archivo parcial)
- **Reordenar la cola arrastrando** los pendientes (el orden arrastrado es el orden de descarga)
- **Pegar varios enlaces a la vez** (Ctrl+V con varias URLs) o arrastrarlos a la ventana
- **Estado «Procesando»** al mezclar/convertir con ffmpeg, y botón **«Mostrar en la carpeta»** al terminar
- 7 tests nuevos de la cola y del parser de progreso (27 en total)

### Changed
- El progreso se envía como **lista completa de la cola** en el evento `queue-updated` (antes un evento parcial por trabajo)
- `start_download` recibe ahora el título junto al enlace y las opciones
- Deshabilitado el drop nativo de Tauri (`dragDropEnabled`) para permitir arrastrar y soltar en HTML dentro del webview (necesario en Windows)

## [0.2.0] - 2026-10-15

### Added
- **Selector de formatos visual**: pestañas Video / Solo audio con las resoluciones reales del enlace
- **Contenedores**: MP4 (H.264+AAC forzado para compatibilidad), MKV y WEBM
- **Formatos de audio**: MP3, M4A/AAC, OGG/Vorbis, OPUS, FLAC, WAV y «Original» (sin recomprimir)
- **Calidad de audio**: 320 / 256 / 192 / 128 kbps
- **Control de volumen** (±20 dB) y **normalización** (loudnorm a -16 LUFS) vía ffmpeg
- **Metadatos y miniatura incrustados** en audio y video
- **Presets de un click**: Máxima calidad, Video 1080p, MP3 320k, FLAC, Audio original
- Resumen de opciones en cada fila de descarga y estado de ffmpeg en Ajustes
- 9 tests nuevos del armado de argumentos (16 en total)

### Notes
- El ajuste de volumen en **video** requiere re-codificar (`--recode-video`): se avisa en la UI
- En MP4 se prefiere H.264 + AAC; en MKV/WEBM se conservan los códecs originales

## [0.1.1] - 2026-10-15

### Added
- **Cookies desde el navegador** (`--cookies-from-browser`) o desde archivo `cookies.txt`, configurables en la nueva vista Ajustes
- **Detección automática de runtime JS** (deno/node/bun/qjs) y solver de desafíos EJS para YouTube
- Vista **Ajustes**: estado del motor (versión de yt-dlp, runtime JS) y botón «Probar conexión» con un video de prueba
- Errores accionables: el anti-bot de YouTube sugiere configurar cookies con acceso directo a Ajustes
- 7 tests unitarios del armado de argumentos y del parser de errores

### Changed
- La UI se organiza en vistas (Descargas / Ajustes) con sidebar navegable

## [0.1.0] - 2026-10-15

### Added
- **UI base estilo Linear**: layout con sidebar, tema oscuro, acento violeta (Tailwind 4)
- **Análisis de enlaces**: pega un link (o Ctrl+V en cualquier parte) → preview con thumbnail, título, canal y duración
- **Descargas en tiempo real**: barra de progreso, velocidad y ETA vía eventos Tauri; cancelar en marcha
- Backend Rust: comandos `probe_url`, `start_download`, `cancel_download`; JobManager con tokio
- Scaffold inicial: Tauri 2 + Svelte 5 (SvelteKit, adapter-static)
- CI en GitHub Actions: svelte-check + build (frontend), fmt + clippy (Rust, Linux y Windows)
- README, CONTRIBUTING y estructura base del repo

### Fixed
- Error 71 (Protocol error) en Wayland con NVIDIA: se deshabilita el renderer dmabuf de WebKitGTK al arrancar
