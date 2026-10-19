# AGENTS.md — Guía para agentes de IA

Instrucciones y contexto para agentes que trabajen en este repositorio. **Mantén este archivo actualizado al cambiar arquitectura, comandos o convenciones.**

## Qué es Downlink

App de **escritorio y Android** (Tauri 2) para descargar video/audio de +1800 sitios con una UI moderna (Svelte 5). El motor de descarga es `yt-dlp`; `ffmpeg` hace conversión, remux, filtros de audio e incrustado de metadatos. Ambos se invocan como **procesos externos**.

- Repo: `github.com/SyberFoxx/downlink`
- Versión actual: **0.10.x** con el port a Android (`feat/android-motores`) en curso hacia la **v1.0**.
- Idioma del proyecto: **español** (comentarios, docs, textos de UI, mensajes de error). Los identificadores de código también están en español cuando es natural.

## Stack

| Capa | Tecnología |
|---|---|
| Shell nativo | Tauri 2 (Rust) |
| Frontend | Svelte 5 (runes) + SvelteKit con `adapter-static` |
| Estilos | Tailwind CSS 4 vía `@tailwindcss/vite` |
| Backend | Rust (edition 2024, tokio, serde) |
| Motor | yt-dlp + ffmpeg (escritorio: del sistema o copia gestionada; Android: empaquetados en el APK) |
| Android | Kotlin (`MainActivity`, `DownloadService`) + JNI, Gradle, NDK |

Estado de navegación: **no hay rutas de SvelteKit en uso**. La app cambia de vista con un estado `view` en `src/routes/+page.svelte` (`"descargas" | "historial" | "apariencia" | "ajustes"`). No agregues rutas nuevas sin discutirlo.

## Estructura

```
downlink/
├── src/                         # Frontend
│   ├── routes/
│   │   ├── +page.svelte         # Layout + switch de vistas (Descargas / Historial / Apariencia / Ajustes)
│   │   ├── +layout.svelte
│   │   └── +layout.ts           # ssr = false, prerender
│   ├── lib/
│   │   ├── api.ts               # Envoltorios tipados de invoke()/listen()/diálogos + rutas Android
│   │   ├── types.ts             # Tipos compartidos con el backend (camelCase)
│   │   ├── options.ts           # Presets, formatos, resumen de opciones
│   │   ├── utils.ts             # Formatos de duración/bytes/velocidad/ETA
│   │   ├── themes.ts            # Catálogo de temas, tokens y derivación desde pywal/wallust
│   │   ├── theme.svelte.ts      # Estado reactivo del tema + aplicación al DOM
│   │   ├── settings.svelte.ts   # Estado reactivo compartido de los ajustes
│   │   ├── downloads.svelte.ts  # Estado compartido de Descargas (probe, opciones, cola) + pegado global
│   │   ├── toast.svelte.ts      # Pila de avisos globales (errores/éxitos)
│   │   ├── components/
│   │   │   ├── Logo.svelte          # Marca (carga static/brand/icon.svg, misma fuente que los iconos)
│   │   │   ├── Sidebar.svelte       # Navegación (iconos) + versión en el pie
│   │   │   ├── PreviewCard.svelte   # Metadatos del probe (layout responsive en móvil)
│   │   │   ├── OptionsPanel.svelte  # Pestañas Video/Audio, presets, volumen
│   │   │   ├── PlaylistPanel.svelte # Selección de vídeos de una playlist
│   │   │   ├── QueueList.svelte     # Sección de cola + drag & drop entre pendientes
│   │   │   ├── DownloadRow.svelte   # Fila de un trabajo (estados y acciones)
│   │   │   └── Toasts.svelte        # Render de los avisos globales
│   │   └── views/
│   │       ├── DownloadsView.svelte # URL, probe, opciones, cola, pegado/arrastre
│   │       ├── HistoryView.svelte   # Galería del historial, búsqueda, re-descarga
│   │       ├── AppearanceView.svelte# Tema, acento, fondo, paleta del escritorio (una sola rejilla)
│   │       └── SettingsView.svelte  # Cookies, concurrencia, motor, «Acerca de», probar conexión
│   └── app.css                  # Tailwind, tokens de diseño y capa de fondo
├── static/
│   ├── brand/icon.svg           # Marca: fuente única de iconos y favicon
│   └── favicon.png              # Derivado de brand/icon.svg (256×256)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Entry point
│   │   ├── lib.rs               # Builder, comandos Tauri, estado, workaround Wayland
│   │   ├── queue.rs             # Cola de trabajos (estados, concurrencia, pausa, orden) + servicio en primer plano
│   │   ├── queue_store.rs       # Persistencia de la cola (queue.json)
│   │   ├── engine.rs            # Resolución de yt-dlp/ffmpeg y su origen (sistema/gestionado/empaquetado)
│   │   ├── updater.rs           # Descarga y auto-update de yt-dlp y ffmpeg (GitHub releases)
│   │   ├── history.rs           # Historial persistido (history.json), dedupe por enlace
│   │   ├── settings.rs          # Settings persistidos, detección de navegadores, EngineInfo
│   │   ├── theme.rs             # Imagen de fondo (data URL) y paleta de pywal/wallust
│   │   ├── tray.rs              # Icono de bandeja (menú, tooltip); solo escritorio
│   │   ├── media_store.rs       # Android: publicar en la galería, abrir archivos y elegir (JNI)
│   │   ├── foreground.rs        # Android: puente del servicio en primer plano (mantiene las descargas)
│   │   ├── share.rs             # Android: recibir enlaces compartidos
│   │   └── ytdlp.rs             # Args, probe, parser de salida, spawn, errores, tests
│   ├── gen/android/             # Proyecto Gradle de Android (se genera con tauri android init)
│   │   └── app/src/main/
│   │       ├── AndroidManifest.xml                     # Permisos y <service> en primer plano
│   │       └── java/com/ramses/downlink/
│   │           ├── MainActivity.kt                     # JNI: MediaStore, pickers, servicio
│   │           └── DownloadService.kt                  # Servicio en primer plano + notificación
│   ├── Cargo.toml
│   └── tauri.conf.json          # Configuración de ventana, bundle e identificador
├── docs/
│   ├── android-v1.md            # Plan y estado del port a Android + ideas post-v1
│   └── download_banner.jpeg     # Banner del README
└── .github/workflows/
    ├── ci.yml                   # CI: frontend + Rust (ubuntu + windows)
    └── release.yml              # Release: instaladores (deb/rpm/AppImage + MSI/NSIS)
```

## Comandos

```bash
# Desarrollo (hot-reload)
npm install
npm run tauri dev

# Chequeos (ejecútalos antes de commitear)
npm run check                                  # svelte-check (tipos)
npm run build                                  # build del frontend
cd src-tauri && cargo fmt --all                # formatea
cd src-tauri && cargo clippy --all-targets -- -D warnings   # lints (deben estar limpios)
cd src-tauri && cargo test                     # tests unitarios

# Empaquetar instaladores (Linux: deb/rpm/AppImage; Windows: MSI/NSIS)
npm run tauri build                            # todo lo que soporte el SO
npm run tauri build -- --bundles deb,rpm       # solo lo que se quiera

# Android (APK firmado arm64)
ANDROID_HOME="$HOME/Android/sdk" ANDROID_SDK_ROOT="$HOME/Android/sdk" \
  NDK_HOME="$HOME/Android/sdk/ndk/28.2.13676358" \
  JAVA_HOME=/usr/lib/jvm/java-17-temurin-jdk \
  npx tauri android build --apk --target aarch64
```

Para empaquetar en Linux hace falta el paquete **`-devel`** de appindicator
(provee el `.pc` de pkg-config) y, para el AppImage, `libfuse2` y `patchelf`
(ver «Trampas técnicas»).

El frontend también soporta `npm run dev` (Vite solo, sin shell nativo — los `invoke()` fallarán).

### Bump de versión

Actualiza **cuatro** sitios (no hay script, hazlo a mano):

1. `package.json` → `"version"`
2. `src-tauri/tauri.conf.json` → `"version"`
3. `src-tauri/Cargo.toml` → `version = "x.y.z"`
4. `src/lib/components/Sidebar.svelte` → texto del pie (`vX.Y.Z-dev`)

Y añade la entrada correspondiente en `CHANGELOG.md` (formato Keep a Changelog, en español).

## Arquitectura y flujo de datos

Regla de oro: **el frontend nunca habla con yt-dlp/ffmpeg directamente**. Todo pasa por comandos Tauri; el progreso sale de Rust con `app.emit(...)` y el frontend solo escucha.

### Comandos Tauri (`src-tauri/src/lib.rs`)

| Comando | Entrada | Salida |
|---|---|---|
| `probe_url` | `url` | `ProbeInfo` (`kind`: `video`/`playlist`, `heights`, `entries`, `truncated`) |
| `start_download` | `url`, `title`, `thumbnail`, `options` | `job_id: u64` (añade a la cola) |
| `pause_download` / `resume_download` / `cancel_download` | `id` | — |
| `retry_download` | `id` | — (reencola un trabajo fallido/cancelado con sus opciones) |
| `reorder_queue` | `ids: Vec<u64>` | — (reordena los pendientes) |
| `clear_finished` | — | — (quita los terminados) |
| `get_queue` | — | `Vec<JobView>` |
| `get_history` | — | `Vec<HistoryEntry>` (más reciente primero) |
| `remove_history` / `clear_history` | `url` / — | — |
| `get_settings` / `save_settings` | `settings` | `Settings` |
| `detect_browsers` | — | `BrowserInfo[]` |
| `engine_info` | — | `EngineInfo` (yt-dlp, runtime JS, ffmpeg) |
| `check_ytdlp_update` | — | `UpdateCheck` (`current`, `latest`, `updateAvailable`) |
| `update_ytdlp` | — | `UpdateCheck` (descarga e instala; progreso por el evento `engine-update`) |
| `check_ffmpeg_update` | — | `UpdateCheck` (comprueba si falta o está desfasado el ffmpeg gestionado) |
| `update_ffmpeg` | — | `UpdateCheck` (descarga ffmpeg+ffprobe; progreso por el evento `engine-update`) |
| `read_wallpaper` | `path` | `String` (la imagen como `data:` URL) |
| `read_system_palette` | — | `SystemPalette \| null` (paleta de pywal/wallust, si existe) |
| `default_download_dir` | — | `String \| null` (carpeta de Descargas del sistema) |
| `tray_available` | — | `bool` (si la bandeja del sistema está disponible) |

### La cola (`queue.rs`)

`Queue` es el estado gestionado por Tauri. Cada trabajo recorre
`queued → downloading → processing → done | error`, más `paused` y `cancelled`.

- **Concurrencia** (`Settings.concurrency`, 1–8, por defecto 2): `pump()` arranca
  pendientes hasta llenar los huecos y vuelve a llamar al terminar cada trabajo.
- **Pausa**: mata el proceso; `yt-dlp` reanuda el archivo `.part` al reanudar, así
  que **no** se empieza de cero (no pasar `--continue` a mano).
- **Orden**: el orden del vector `jobs` es el orden de despacho de los pendientes.
  `reorder_queue` lo cambia; la UI solo reordena los que están `queued`.
- **Reintentar** (`retry_download`): reencola un trabajo `error` o `cancelled`
  conservando sus opciones. Si su proceso aún está vivo (pausa/cancelación
  reciente) no hace nada, para no pisar el `finish` en curso.
- **Quitar**: `cancel_download` marca `cancelled` los trabajos vivos y **quita de
  la lista** los que ya terminaron (`done`/`error`/`cancelled` sin proceso activo).
- **Persistencia** (`Settings.persist_queue`, activa por defecto): `queue_store.rs`
  escribe `queue.json` (en `app_config_dir()`) tras cada cambio de cola, con los
  trabajos no terminados y `next_id`. Al arrancar, `Queue::restore()` los
  recupera: lo que estaba en vuelo vuelve como `queued` (yt-dlp reanuda el
  `.part`) y lo pausado sigue pausado, siempre con el progreso a cero. Al
  desactivar el ajuste, `save_settings` borra el archivo.
- **Android (`sync_foreground` + `foreground.rs`)**: al cambiar el estado de la
  cola, `sync_foreground` arranca/actualiza/para el servicio en primer plano y su
  notificación de progreso. `foreground_summary` (función pura, con tests)
  calcula el texto y el porcentaje medio de la notificación. En Android,
  `notify_finished` avisa al terminar **aunque la ventana tenga el foco** (la app
  puede estar en segundo plano).
- **Publicación en Android (`publish_if_android`)**: al terminar, el archivo se
  copia a la galería con `media_store::publish_to_media_store` y `displayPath`
  pasa a decir «Guardado en la galería del dispositivo».

### Bandeja, notificaciones y carpeta de descarga

- **Bandeja** (`tray.rs`, módulo con `#[cfg(desktop)]`): icono con menú
  «Mostrar Downlink» / «Salir», clic izquierdo para mostrar la ventana y tooltip
  con el estado de la cola. `tray::sync_tooltip()` solo escribe si el texto
  cambió, así que se llama en cada `emit()`. Si el sistema no tiene bandeja,
  `setup()` falla sin romper nada y la app sigue normal (bandera `tray_available`).
  El icono usa `app.default_window_icon()`.
- **Cerrar a la bandeja** (`Settings.close_to_tray`, desactivado por defecto):
  `on_window_event` llama `api.prevent_close()` y oculta la ventana, **solo** si
  la bandeja arrancó de verdad (comprobación de seguridad con `tray_available`).
- **Notificaciones** (`tauri-plugin-notification`, vía `NotificationExt`):
  `Queue::notify_finished()` avisa al terminar o fallar, pero **solo** con
  `Settings.notify_on_finish` activo y, en escritorio, **solo si la ventana no
  tiene el foco** (si está delante, el usuario ya lo ve).
- **Carpeta de descarga** (`Settings.download_dir`): `settings::download_dir()`
  resuelve la carpeta configurada (creándola con `create_dir_all`) o, si no hay,
  la del sistema. El comando `default_download_dir` la expone para que la UI
  muestre la referencia. Es donde `pump()` le pasa el `dir` a `spawn_process`.

### Evento de cola

Canal único `queue-updated` (`QUEUE_EVENT`) con **la lista completa** de trabajos
(`Vec<JobView>`): el frontend la sustituye entera, sin fusionar.

```jsonc
{ "id": 1, "url": "…", "title": "…", "thumbnail": "https://…", "options": { … },
  "status": "queued | downloading | processing | paused | done | error | cancelled",
  "percent": 42.5, "downloadedBytes": 123, "totalBytes": 456, "speedBps": 789,
  "etaSecs": 10, "filePath": "/home/…/video.mp4", "message": null }
```

Se emite como máximo cada 200 ms entre ticks de progreso.

Protocolo de línea sobre stdout de yt-dlp (en `ytdlp.rs` → `parse_line`):
- `__DL__<descargado>|<total>|<total_estimado>|<velocidad>|<eta>` → progreso (el
  porcentaje y el ETA se calculan en Rust: `progress.percent` de yt-dlp da `NA`)
- `__FILE__<ruta>` → ruta final del archivo
- líneas `[ExtractAudio]`, `[Merger]`, `[Metadata]`… → estado «procesando»

### Estado y avisos en el frontend

- El estado de Descargas (enlace, `probe`, opciones, selección de la playlist y
  cola) vive en `downloads.svelte.ts` para que **no se pierda al cambiar de
  vista**. La cola se suscribe una sola vez desde `+page.svelte` (raíz).
- El **pegado y el arrastre de enlaces son globales** (también en `+page.svelte`):
  desde otra vista la app salta a Descargas y analiza el enlace. Los campos de
  texto se ignoran; el arrastre interno de la cola lleva el tipo MIME
  `application/x-downlink-queue` (`QUEUE_DRAG_TYPE`) para no confundirse con un
  enlace soltado.
- `toast.svelte.ts` + `Toasts.svelte` son los avisos emergentes: los fallos sin
  sitio natural en la UI (p. ej. `settings.save()`, que devuelve `false`) se
  muestran ahí en vez de perderse en silencio.
- **Transiciones y skeletons**: el switch de vistas va dentro de un `{#key view}`
  con `in:fly` (se salta si el sistema pide reducir movimiento). Al cambiar de
  vista el contenedor con scroll vuelve arriba. `downloads.queueLoaded` distingue
  «aún no sé si hay cola» de «cola vacía», para que `QueueList` muestre filas
  fantasma y no un parpadeo de estado vacío; `HistoryView` hace lo propio con su
  rejilla de tarjetas mientras carga. En `app.css` hay una regla global
  `@media (prefers-reduced-motion: reduce)` que apaga animaciones y transiciones.

### Playlists

`probe` usa `--flat-playlist` (rápido, sin extraer cada vídeo) y devuelve
`kind: "playlist"` con `entries` (`id`, `title`, `url`, `duration`, `thumbnail`).
Las entradas de YouTube solo traen el id, así que `entry_url` construye
`https://www.youtube.com/watch?v=<id>` cuando la URL no es absoluta. La descarga
sigue siendo por vídeo suelto (`--no-playlist` en `build_download_args`).

Dos salvaguardas en `playlist_entries` / `probe_args`:

- **Deduplicación por `id`**: las radios/mixes de YouTube repiten vídeos. Con ids
  repetidos las claves del `{#each}` chocaban y rompían la vista.
- **Pausa entre peticiones**: `--sleep-requests` para no provocar el anti-bot.

### Historial (`history.rs`)

`history.json` en `app_config_dir()`, deduplicado por enlace y ordenado por fecha
(más reciente primero). La miniatura se guarda como URL, no como bytes.

### Esquema de argumentos

`ytdlp.rs::build_args` es la única fuente de argumentos. Añadir una opción
significa: campo en `DownloadOptions` (Rust + `types.ts`), su uso en
`build_args`, su control en `OptionsPanel.svelte` y su resumen en `options.ts`.

### Motor (yt-dlp y ffmpeg)

`engine.rs` resuelve la ruta de cada binario y su **origen**, en este orden:

1. Variable de entorno (`DOWNLINK_YTDLP` / `DOWNLINK_FFMPEG`).
2. Copia **gestionada** por la app en `<app_data>/bin` (base para descargar/auto-actualizar).
3. **Sidecar** junto al ejecutable (mecanismo `externalBin` de Tauri).
4. `PATH` del sistema.
5. **Android**: `nativeLibraryDir` del APK, donde van los intérpretes empaquetados
   como `.so` (`libpython.so`, `libffmpeg.so`, `libffprobe.so`, `libquickjs.so`).
   `engine_info` informa de que ya están actualizados, así que no se ofrecen
   descargas.

`EngineInfo` devuelve `ytdlpSource`/`ytdlpPath` y `ffmpegSource`/`ffmpegPath` para
mostrarlos en Ajustes. Si ffmpeg **no** viene del `PATH`, `common_args` añade
`--ffmpeg-location <ruta>` al binario (yt-dlp busca `ffprobe` en su misma carpeta;
en el `PATH` lo encuentra solo). Por eso la copia gestionada instala `ffmpeg` y
`ffprobe` **juntos** en `<app_data>/bin`.

### Auto-update de yt-dlp (`updater.rs`)

Descarga la copia gestionada de yt-dlp desde los *releases* oficiales de GitHub
(`api.github.com/.../releases/latest` → asset de la plataforma) a
`<app_data>/bin`. Antes de sustituir el binario:

1. **SHA-256** contra el `SHA2-256SUMS` del release (si no se puede leer, se omite).
2. **Ejecución real**: el binario temporal corre `--ignore-config --version`; si no
   arranca, se descarta.

Solo entonces se reemplaza con `rename` (atómico). Se lanza:

- **A mano** desde Ajustes → `update_ytdlp` (muestra progreso con el evento `engine-update`).
- **Automático** al arrancar (`auto_update`), si `Settings.autoUpdateYtdlp` está activo
  y no hay `DOWNLINK_YTDLP` definida. Nunca es fatal: si falla, la app sigue igual.

Solo se soportan los assets publicados como binario suelto (`yt-dlp_linux`,
`yt-dlp_linux_aarch64`, `yt-dlp.exe`, `yt-dlp_x86.exe`, `yt-dlp_arm64.exe`,
`yt-dlp_macos`). `yt-dlp_linux_armv7l` solo se publica dentro de un `.zip` y **no**
se ofrece (aunque ffmpeg sí desempaqueta, ver abajo, yt-dlp se limita a los binarios sueltos).

### Descarga de ffmpeg (`updater.rs`)

`ffmpeg` (y `ffprobe`, que se instala a su lado) se descarga de los builds estáticos
de [BtbN/FFmpeg-Builds](https://github.com/BtbN/FFmpeg-Builds), tag móvil `latest`,
y se descomprime a `<app_data>/bin`. Igual que yt-dlp: se comprueba que el binario
arranca antes de dar por buena la instalación.

### Temas y apariencia (`themes.ts`, `theme.rs`)

La interfaz **no usa colores de Tailwind directamente**: usa *tokens* semánticos
(`--canvas`, `--surface`, `--elevated`, `--line`, `--ink`, `--ink-strong`,
`--ink-soft`, `--ink-muted`, `--ink-faint`, `--accent`, `--accent-hover`,
`--accent-soft`, `--accent-ink`, `--danger`, `--danger-bg`, `--ok`, `--warn`).

- En `app.css` se registran con `@theme inline { --color-canvas: var(--canvas); … }`,
  de modo que las utilidades son `bg-canvas`, `text-ink-muted`, `border-line`,
  `text-accent`, `bg-danger-bg/30`, etc. (las opacidades usan `color-mix`, que ya
  usaba Tailwind para sus propias `/NN`). El `:root` de `app.css` trae el tema oscuro
  como valor por defecto para que no haya parpadeo al arrancar.
- **Al programar usa siempre los tokens**, nunca `gray-*`/`violet-*`/`red-*`… Si
  necesitas un color nuevo, añade el token en los tres sitios: `themes.ts`
  (`ThemeTokens` + `TOKEN_VARS`), el `@theme inline` de `app.css` y los 10 temas.
- `themes.ts` es el **catálogo** (10 temas, cada uno con su `appearance` dark/light y
  sus tokens). Añadir un tema es añadir un objeto a `THEMES`.
- `theme.svelte.ts` es el **estado reactivo**: carga los ajustes, sigue el modo
  claro/oscuro del sistema (`prefers-color-scheme`), lee la paleta del escritorio y
  aplica los tokens al `<html>` con un `$effect.root`. `themeFromPalette()` deriva un
  tema completo desde los 16 colores de pywal/wallust.
- El modo `themeId = "system"` es **automático**: usa `downlink-dark`/`downlink-light`
  según el sistema y, si hay paleta de pywal/wallust, la adopta. El tema por defecto
  sigue siendo `downlink-dark`.
- **Imagen de fondo**: `theme.rs::read_wallpaper` la lee y la devuelve como `data:`
  URL (tope 16 MB) para no abrir el protocolo de assets de Tauri; el frontend la pinta
  en `.app-wallpaper` (capa fija `z-index:-1` con `--wallpaper-blur`) y un `::after`
  usa `--wallpaper-dim` como velo del color del lienzo. **En móvil el desenfoque
  funciona igual** (no lo desactives con `filter: none`).
- **Paleta del escritorio**: `theme.rs::read_system_palette` lee
  `~/.cache/wallust/colors.json` y luego `~/.cache/wal/colors.json` con un recorrido
  tolerante del JSON (pywal y wallust no coinciden al 100%). En Android no hay pywal.
- Los ajustes los comparte una **única fuente de verdad** (`settings.svelte.ts`), para
  que guardar en Ajustes no pise lo de Apariencia ni al revés.
- **Apariencia (móvil)**: los presets de paleta van en **una sola rejilla**; evita
  huecos vacíos entre grupos (era un fallo visual en pantalla estrecha).

### Android (Kotlin + JNI)

El proyecto nativo vive en `src-tauri/gen/android/` (se crea con `tauri android init`,
no lo regeneres a la ligera). Piezas clave:

- **`MainActivity.kt`**: métodos `@Keep` que Rust llama por JNI (`with_webview(...)
  .jni_handle().exec(...)`, siempre en el hilo del webview). Hace de puente para
  MediaStore (`publishToMediaStore`, `openDownloaded`), los *pickers*
  (`pickWallpaper`, `pickCookiesFile`, `copyToPrivateStorage`) y el servicio
  (`startDownloadService`, `updateDownloadService`, `stopDownloadService`).
- **`DownloadService.kt`**: servicio en primer plano (`foregroundServiceType="dataSync"`)
  con notificación persistente en el canal `downlink_downloads`. La primera llamada
  debe ser `startForegroundService`; las siguientes, `startService`. Un `AtomicBool`
  `RUNNING` en `foreground.rs` recuerda si ya está arrancado.
- **`media_store.rs` / `foreground.rs`**: envoltorios Rust del JNI. Los *pickers*
  copian el archivo elegido al almacenamiento privado (`Android/data/<id>/files/`),
  porque el permiso `content://` es temporal.
- **Al terminar, los archivos se copian a la galería** (`/sdcard/Movies/Downlink/` en
  vídeo, `Music` en audio) y se pueden abrir con el visor del sistema.

**Reglas que no se pueden romper** (todas causaron fallos reales en el dispositivo):

1. **Las firmas JNI deben coincidir con Kotlin al carácter**: un `stopDownloadService`
   declarado como `()V` y llamado como `(Ljava/lang/String;)V` lanza
   `NoSuchMethodError` y **mata la app**. Usa `foreground.rs::call_string`/`call_void`
   según corresponda y comprueba las firmas en el `.so` (`llvm-nm`/`readelf`).
2. **R8 conserva los métodos JNI solo con `-keepclassmembers` literal** en
   `proguard-rules.pro`; si añades un método nuevo, añádelo también allí.
3. **Los comentarios de bloque en Kotlin anidan**: un `/*` suelto (por ejemplo un glob
   `*/*`) dentro de un KDoc `/** … */` rompe la compilación.
4. **Nada de `unwrap()` en las rutas JNI**: si el handle no está listo, se devuelve
   error y se avisa, nunca se paniquea.

### Marca e iconos (`static/brand/icon.svg`, `Logo.svelte`)

La marca es una **flecha descendente entrando en una bandeja** sobre un cuadrado
con degradado **violeta→índigo** (`#8b5cf6` → `#7c3aed` → `#4338ca`).

- **`Logo.svelte` carga `static/brand/icon.svg`** (fuente única): no dupliques el
  SVG a mano en el componente. Los colores son **fijos a propósito**: son identidad
  de marca, no siguen al tema elegido.
- **Regenerar los iconos** (al tocar la marca, desde la raíz del repo):

  ```bash
  inkscape static/brand/icon.svg --export-type=png \
    --export-filename=/tmp/downlink-icon.png --export-width=1024 --export-height=1024
  npx tauri icon /tmp/downlink-icon.png   # reescribe src-tauri/icons/* (incluido Android)
  inkscape static/brand/icon.svg --export-type=png \
    --export-filename=static/favicon.png --export-width=256 --export-height=256
  ```

  `npx tauri icon` también genera los juegos de iOS/Android en
  `src-tauri/icons/{ios,android}/`.
- Los iconos de la navegación son **Heroicons 24 outline** (paths en línea en
  `Sidebar.svelte`, `stroke-width` 1.75). Mantén ese estilo si añades vistas.
- `tauri.conf.json`: `productName` es `Downlink` (nombra bundles, instaladores y
  la entrada `Name` del `.desktop`; el binario sigue saliendo de Cargo, `downlink`).
  El bundle lleva metadatos (`publisher`, `copyright`, `category`, `license`,
  `licenseFile`, `homepage`, `shortDescription`, `longDescription`). El
  `identifier` **no** se toca sin migrar datos: de él cuelga `app_config_dir()`.

## Convenciones de código

- **Español** en comentarios, docs, textos de UI y mensajes de error. Mensajes claros y accionables (dicen qué hacer, no solo qué falló).
- **Nada de `unwrap()`** en rutas que manejen entrada del usuario: usa `Result` + mensajes.
- Rust: `cargo fmt` y `clippy -D warnings` deben estar limpios. Tests unitarios junto al código (`#[cfg(test)] mod tests`).
- Ninguna promesa se queda sin gestionar: si una acción puede fallar (guardar
  ajustes, un comando Tauri…), se captura y se avisa (toast o mensaje en
  contexto). **Nada de errores silenciosos.**
- Frontend: Svelte 5 con **runes** (`$state`, `$derived`, `$props`, `$bindable`, `$state.snapshot`). No uses stores ni sintaxis de Svelte 4.
- Al enviar opciones al backend desde estado reactivo usa `$state.snapshot(options)` (los proxies no serializan bien).
- Commits: [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, `docs:`).
- Estilo visual: tokens semánticos (ver «Temas y apariencia»); nunca colores de Tailwind a pelo. Tema por defecto oscuro tipo Linear.

## Cosas específicas del entorno y del motor

- **Wayland + NVIDIA**: `lib.rs` define `WEBKIT_DISABLE_DMABUF_RENDERER=1` al arrancar (Linux). Sin esto, WebKitGTK falla con `Error 71 (Protocol error)`. No lo quites.
- **YouTube anti-bot**: requiere cookies + runtime JS (node/deno) + solver EJS (`--remote-components ejs:github`, solo en dominios de YouTube). Los errores se traducen a mensajes que apuntan a Ajustes. En Android el runtime JS es QuickJS (empaquetado) y las cookies hay que importarlas desde un `cookies.txt` de escritorio.
- **ffmpeg**: obligatorio para convertir formatos y aplicar filtros de audio.
- Variables de entorno útiles (override, sobre todo en tests): `DOWNLINK_YTDLP` y `DOWNLINK_FFMPEG` (rutas de los binarios), `DOWNLINK_JS_RUNTIME` (runtime forzado), `WEBKIT_DISABLE_DMABUF_RENDERER`.
- **Config y ajustes persistidos**: `settings.json` y `history.json` dentro de `app_config_dir()` (en Linux, `~/.config/<bundle-id>/`; en Android, `Android/data/<id>/files/`).
- Los archivos se descargan en `download_dir()` con patrón `%(title)s [%(id)s].%(ext)s`.
- **`dragDropEnabled: false`** en `tauri.conf.json`: con el drop nativo de Tauri activo el
  webview no dispara el drag & drop HTML5 interno (rompe el reordenamiento de la cola, sobre
  todo en Windows). El arrastre de enlaces se resuelve en HTML5 en `DownloadsView`.

### Trampas técnicas ya resueltas (no las reintroduzcas)

1. **El merge de yt-dlp usa `-c copy`** → no acepta filtros de audio (`-af`). Por eso:
   - Audio: filtros vía `--postprocessor-args "ExtractAudio+ffmpeg:-af …"`.
   - Video: filtros obligan a `--recode-video <cont> --postprocessor-args "VideoConvertor+ffmpeg:-af …"` (re-codifica; se avisa en la UI).
2. **MP4 poco compatible por defecto** (yt-dlp puede elegir AV1+Opus): con contenedor MP4 se fuerza `-S vcodec:h264,acodec:aac`.
3. **«Original (sin recomprimir)» + filtros** es imposible (los filtros exigen re-codificar): el backend fuerza `m4a` y la UI desactiva el control de volumen para ese caso.
4. **`volume=0` no debe generar filtro** (evita re-codificar de más).
5. La miniatura se incrusta con ffmpeg; **no se necesita AtomicParsley**.
6. **Empaquetar en Linux exige el paquete `-devel` de appindicator**: `tauri-cli`
   consulta pkg-config para calcular las dependencias de los `.deb`/`.rpm` y
   paniquea con «Can't detect any appindicator library» (`rust.rs`) si no
   encuentra el `.pc`, aunque el `cargo build` funcione (Tauri carga
   appindicator con `dlopen`, así que solo hacen falta los runtime). Fedora:
   `libayatana-appindicator-gtk3-devel`; Debian/Ubuntu:
   `libayatana-appindicator3-dev`. Para el AppImage, además `libfuse2` y
   `patchelf`.
7. **La AppImage inyecta `LD_LIBRARY_PATH`**: su lanzador exporta
   `LD_LIBRARY_PATH=$APPDIR/usr/lib:…` y empaqueta sus propias `libcrypto.so.3`,
   `libssh.so.4`, `libcurl-gnutls.so.4` (más antiguas que las del sistema). Si un
   proceso hijo las hereda, **`ffmpeg` y `node` del sistema no arrancan**
   (`version 'OPENSSL_3.4.0' not found`). Síntomas (solo en la AppImage, nunca en
   `tauri dev`): «Postprocessing: ffmpeg not found» al descargar, YouTube pidiendo
   cookies (sin `node` no se resuelven los desafíos JS) y Ajustes diciendo «no
   detectado» aunque ffmpeg esté instalado. Por eso **todo proceso externo se lanza
   con `engine::command()`**, que sanea `LD_LIBRARY_PATH`/`LD_PRELOAD` quitando las
   rutas bajo `APPDIR` (fuera de una AppImage no hace nada). No vuelvas a usar
   `Command::new()` directo para yt-dlp/ffmpeg.
8. **`ffmpeg_update_available` no da por bueno un ffmpeg que no arranca**: si el
   binario resuelto no reporta versión, se considera que no sirve y Ajustes ofrece
   la copia gestionada. Antes un ffmpeg roto bloqueaba cualquier salida (ni
   convertía ni dejaba descargar otro).
9. **Android exige `extractNativeLibs=true`** (en
   `gen/android/app/build.gradle.kts`): los `.so` del motor (Python, ffmpeg, QuickJS)
   no se pueden `dlopen` desde el APK comprimido. Comprueba con
   `aapt2 dump xmltree --file AndroidManifest.xml <apk> | grep extractNativeLibs`.
10. **El `<option value={null}>` de Svelte llega como la cadena `"null"`**:
    para «la mejor calidad» usa un centinela (`BEST`) y `onchange`, no `bind:value`.
11. **En Android la app no puede leer `content://`/`file://` ajenos** (galería,
    Descargas) una vez vuelve la actividad: por eso los *pickers* **copian** el
    archivo al almacenamiento privado en vez de guardar la URI.

## CI

`.github/workflows/ci.yml` — dos jobs en push/PR a `main`:
- **frontend** (ubuntu): `npm ci` → `npm run check` → `npm run build`.
- **rust** (ubuntu + windows, matriz): `cargo fmt --check` → `cargo clippy -D warnings` → `cargo test` → `cargo check`.

El código `#[cfg(target_os = "android")]` **no lo ve el CI de escritorio**: para
comprobarlo hace falta el target del NDK:

```sh
cargo check --target aarch64-linux-android
```

Verifica el CI con: `gh run watch <run-id> --exit-status`.

`.github/workflows/release.yml` — empaquetado y publicación:

- Se dispara al etiquetar `v*` (o a mano con `workflow_dispatch`, que solo sube
  los artefactos sin publicar — sirve para probar el workflow).
- Job **bundle** (matriz ubuntu + windows): `npm run tauri build -- --bundles …`
  → `.deb`, `.rpm` y AppImage en Linux; `.msi` y NSIS en Windows. En Linux
  instala además `libfuse2` y `patchelf` (necesarios para el AppImage). Los
  binarios se suben como artefactos.
- Job **release** (solo con tag, `permissions: contents: write`): junta los
  artefactos con `download-artifact` y crea la release subiendo los ficheros
  (`gh release create <tag> "${assets[@]}"`, con los nombres sacados de
  `find dist -type f`; o `gh release upload --clobber` si la release ya existe).
  **Ojo**: los artefactos llegan con la subcarpeta de cada plataforma
  (`dist/deb/…`, `dist/rpm/…`, `dist/msi/…`), así que `dist/*` expande a
  directorios y `gh` falla con «read dist/rpm: is a directory».
- **Pendiente (v1.0)**: construir el APK arm64 en el workflow.

## Estado y roadmap

Ver `README.md` → sección Roadmap (fuente de verdad del plan) y `CHANGELOG.md` (lo ya entregado).

El plan del port a Android vive en **`docs/android-v1.md`** (bloqueantes, adaptaciones,
fases y estado por fase, más las ideas aparcadas para después de la v1).

Resumen:
- **v0.1 / v0.1.1** ✅ — cimientos, cookies del navegador, EJS, vista Ajustes.
- **v0.2** ✅ — selector de formatos, contenedores, formatos de audio, volumen/normalización, presets.
- **v0.3** ✅ — cola inteligente: concurrencia, pausa/reanudar, reordenar arrastrando, pegado múltiple.
- **v0.4** ✅ — playlists: detección, listado, selección y encolado.
- **v0.5** ✅ — historial con galería, búsqueda y re-descarga + persistencia local.
- **v0.6** ✅ — recorte de clips (`--download-sections`) y subtítulos (descarga e incrustado).
- **v0.7** ✅ — madurez del motor: detección de sidecars, auto-update de yt-dlp y descarga de ffmpeg.
- **v0.8** ✅ — sistema de temas (tokens, 10 temas, acento, fondo, paleta de pywal/wallust).
- **v0.9** ✅ — carpeta configurable, SponsorBlock, cola persistente, notificaciones, bandeja, reintentar, toasts, pegado global y marca propia.
- **v0.10** ✅ — instaladores deb/rpm/AppImage y MSI/NSIS publicados en GitHub Releases por el workflow `release.yml`.
- **v1.0** 🚧 — **Android (arm64)**: funcional en dispositivo (motores empaquetados, galería,
  servicio en primer plano, *pickers*, APK firmado). Queda el CI de Android y el pulido que
  salga de probar en el teléfono. Ver `docs/android-v1.md`.

## Al terminar un cambio

1. `cargo fmt` + `clippy -D warnings` + `cargo test` limpios.
2. `npm run check` y `npm run build` sin errores.
3. Si toca código `#[cfg(target_os = "android")]`, `cargo check --target aarch64-linux-android`.
4. Actualiza `CHANGELOG.md` si es un cambio visible para el usuario.
5. Si toca arquitectura/comandos/convenciones, **actualiza este `AGENTS.md`**.
