# Changelog

Todo lo notable de Downlink se documenta aquí.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/),
y el proyecto adhiere a [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

### Added
- **Reintentar**: los trabajos fallidos o cancelados tienen botón «Reintentar», que los reencola con las mismas opciones (nuevo comando `retry_download`)
- **Avisos emergentes (toasts)** en la esquina inferior derecha: los fallos que antes pasaban en silencio (sobre todo al guardar ajustes) ahora se ven
- **Pegar o arrastrar enlaces desde cualquier vista**: estando en Historial o Ajustes, un enlace pegado o soltado salta a Descargas y se analiza al momento
- Anillo de foco visible al navegar con teclado, `aria-current` en la barra lateral y etiquetas accesibles en los campos de búsqueda y enlace
- **Marca propia**: logotipo de Downlink (flecha descendente sobre una bandeja, con degradado violeta→índigo) en la barra lateral, favicon e **iconos de la aplicación** para Linux, Windows y macOS — ya no son los de ejemplo de Tauri
- **Iconos en la navegación**: cada vista de la barra lateral tiene el suyo
- **Transiciones entre vistas** (un desvanecido breve) y **esqueletos de carga** en la cola y en el historial, que sustituyen al parpadeo del estado vacío mientras se le pregunta al backend
- Metadatos del paquete y nombre visible de la aplicación: «Downlink» en instaladores, menús y la entrada `.desktop`, con descripción, categoría, editor, licencia y web

### Changed
- Las animaciones, transiciones y giros se desactivan si el sistema pide **reducir el movimiento**
- Al cambiar de vista, el contenido vuelve arriba en lugar de heredar la posición de scroll de la vista anterior
- El README ya no dice que yt-dlp y ffmpeg van «como sidecars»: se invocan como procesos externos (del sistema o copias que gestiona la app)

### Fixed
- El estado de Descargas (enlace analizado, selección de la playlist y opciones) ya no se pierde al cambiar de vista y volver
- Ajustes ya no se queda con «Guardar» y «Probar conexión» deshabilitados si falla la lectura del equipo al abrir la vista
- Guardar ajustes o apariencia sin backend ya no genera errores sin gestionar: se avisa con un toast
- Las cajas de error se pueden cerrar y los botones de solo icono de la cola tienen nombre accesible
- La ✕ de un trabajo ya terminado (error o cancelado) ahora lo quita de la lista en vez de no hacer nada
- El resaltado de «suelta para añadir» ya no aparece al reordenar la cola arrastrando

## [0.8.0] - 2026-10-15

### Added
- **Sistema de temas**: la interfaz pasa a usar *tokens* de diseño (variables CSS) en vez de colores fijos, así que cambiar de tema repinta toda la app sin recompilar
- **Nueva vista Apariencia** en la barra lateral, con tema, color de acento e imagen de fondo
- **10 temas incluidos**: Oscuro (el de siempre), Claro, Catppuccin Mocha, Catppuccin Latte, Nord, Dracula, Gruvbox, Tokyo Night, Rosé Pine y Solarized Light
- **Modo automático**: sigue el claro/oscuro del sistema y, si encuentra una paleta de **pywal o wallust**, imita los colores de tu escritorio (pensado para Hyprland y demás escritorios con *ricing*)
- **Color de acento personalizable**: doce colores sugeridos más un selector libre, aplicable sobre cualquier tema
- **Imagen de fondo** propia (wallpaper) con control de intensidad y desenfoque, por detrás de los paneles translúcidos
- Nuevos comandos `read_wallpaper` (devuelve la imagen como `data:` URL, sin abrir el protocolo de assets) y `read_system_palette` (lee `~/.cache/wallust` o `~/.cache/wal`)
- Nuevo módulo `theme.rs` con la codificación base64, la detección de la paleta del escritorio y 7 tests
- Nuevos archivos frontend: `themes.ts` (catálogo), `theme.svelte.ts` y `settings.svelte.ts` (estado compartido)

### Changed
- Los ajustes de Ajustes y Apariencia comparten una única fuente de verdad, así que guardar en una vista ya no pisa la otra
- Todos los componentes usan ahora utilidades semánticas (`bg-surface`, `text-ink-muted`, `border-line`, `text-accent`…) en lugar de colores de Tailwind concretos

### Notes
- El tema por defecto sigue siendo el oscuro de siempre; nada cambia si no entras en Apariencia
- El modo automático depende de que el webview exponga `prefers-color-scheme`; en Linux lo habitual es que siga el tema GTK
- La imagen de fondo se lee y se sirve como `data:` URL (máximo 16 MB) para que funcione igual en desarrollo y en producción

## [0.7.1] - 2026-10-15

### Added
- **Descarga y actualización de ffmpeg**: Downlink puede bajar su propia copia de `ffmpeg` y `ffprobe` (builds estáticos de [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds)) a `<app_data>/bin`, con las mismas garantías que yt-dlp: verificación **SHA-256** contra `checksums.sha256`, ejecución de prueba (`-version`) y sustitución atómica
- **Instalación automática si falta** (activable en Ajustes, activada por defecto): al arrancar, si no se encuentra ningún ffmpeg, se descarga en segundo plano. Si ya tienes ffmpeg del sistema, se respeta y no se toca
- **Actualización manual desde Ajustes**: la sección Motor muestra el estado de ffmpeg, permite buscar/descargar/actualizar a mano con barra de progreso, y refresca versión y origen al terminar
- Nuevos comandos `check_ffmpeg_update` y `update_ffmpeg`, y nueva opción `autoDownloadFfmpeg` en los ajustes persistidos
- Nuevo componente `EngineUpdateRow.svelte` reutilizado por los dos motores (yt-dlp y ffmpeg)
- 8 tests nuevos del actualizador (16 en `updater.rs`, 67 en total) más un test de integración opcional que valida la extracción contra un archivo real de BtbN

### Changed
- El evento `engine-update` incluye ahora el motor (`yt-dlp` o `ffmpeg`) para que cada fila de Ajustes muestre solo su progreso

### Notes
- Los builds de ffmpeg son grandes (~150 MB comprimidos, ~340 MB `ffmpeg` + `ffprobe` en disco) y se regeneran a diario, así que **no se refresca solo**: la descarga automática es solo para cuando falta ffmpeg y la actualización de una copia existente es manual
- Solo se ofrecen builds de ffmpeg para **Linux y Windows** (x86_64 y arm64); en macOS u otras plataformas se usa el ffmpeg del sistema
- El archivo descargado se desempaqueta en Rust (`tar`+`lzma-rs` para `.tar.xz`, `zip` para `.zip`), sin depender de `tar`, `xz` ni `unzip` del sistema

## [0.7.0] - 2026-10-15

### Added
- **Auto-update de yt-dlp**: Downlink descarga su propia copia de yt-dlp desde los *releases* oficiales de GitHub y la mantiene al día, sin depender del `PATH` del sistema
- **Verificación en dos pasos**: el binario descargado se comprueba contra el `SHA2-256SUMS` del release y se ejecuta (`--version`) antes de sustituir nada; la instalación es atómica
- **Actualización desde Ajustes**: la sección Motor muestra si hay versión nueva, permite actualizar a mano (con barra de progreso) y refresca la versión/origen al terminar
- **Comprobación automática al arrancar** (activable en Ajustes, activada por defecto): si hay versión nueva, se descarga en segundo plano sin bloquear la interfaz
- Nueva opción `autoUpdateYtdlp` en los ajustes persistidos y nuevo comando `update_ytdlp`; el progreso se emite por el evento `engine-update`
- 8 tests nuevos del actualizador (59 en total)
- El CI ahora ejecuta `cargo test`

### Changed
- El motor prefiere la copia gestionada por la app (`<app_data>/bin`) por delante del sidecar y del `PATH`, así que la actualización automática sustituye al `yt-dlp` del sistema sin tocarlo

### Notes
- Si defines `DOWNLINK_YTDLP`, esa ruta tiene prioridad y la actualización automática se desactiva (útil en desarrollo)
- Solo se ofrecen los binarios sueltos publicados por yt-dlp; plataformas que solo se publican en `.zip` (p. ej. Linux armv7l) no se actualizan desde aquí

## [0.6.1] - 2026-10-15

### Added
- **Detección del origen del motor**: Ajustes ahora indica de dónde sale cada binario (sistema, gestionado por Downlink, incluido con la app o variable de entorno), con su ruta
- **Soporte de sidecars**: el motor busca yt-dlp/ffmpeg también en la carpeta gestionada de la app (`app_data/bin`) y junto al ejecutable, antes de caer al `PATH`
- Si ffmpeg no está en el `PATH` (copia gestionada o sidecar), se pasa `--ffmpeg-location` a yt-dlp automáticamente
- Nuevo módulo `engine.rs` con la resolución de binarios y su origen (4 tests nuevos, 51 en total)

## [0.6.0] - 2026-10-15

### Added
- **Recorte de clips**: define un inicio y un fin para descargar solo el fragmento que quieres (p. ej. `1:30` → `2:15`) con `--download-sections`
- **Subtítulos**: descarga los subtítulos del vídeo en los idiomas que indiques (`es,en`…), incluye los automáticos y permite **incrustarlos** en el archivo (MP4 → `mov_text`, MKV → `webvtt`) o dejarlos como `.srt`
- Los tiempos del recorte aceptan `mm:ss`, `h:mm:ss` o segundos sueltos, con validación en la UI
- 9 tests nuevos de argumentos (47 en total)

### Changed
- Las opciones de descarga (recorte y subtítulos) se guardan con el historial y se reutilizan al re-descargar

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
