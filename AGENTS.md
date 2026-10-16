# AGENTS.md — Guía para agentes de IA

Instrucciones y contexto para agentes que trabajen en este repositorio. **Mantén este archivo actualizado al cambiar arquitectura, comandos o convenciones.**

## Qué es Downlink

App de escritorio (Tauri 2) para descargar video/audio de +1800 sitios con una UI moderna (Svelte 5). El motor de descarga es `yt-dlp`; `ffmpeg` hace conversión, remux, filtros de audio e incrustado de metadatos. Ambos se invocan como **procesos externos** (todavía no empaquetados como sidecars).

- Repo: `github.com/SyberFoxx/downlink` (privado)
- Versión actual: **0.7.1**
- Idioma del proyecto: **español** (comentarios, docs, textos de UI, mensajes de error). Los identificadores de código también están en español cuando es natural.

## Stack

| Capa | Tecnología |
|---|---|
| Shell nativo | Tauri 2 (Rust) |
| Frontend | Svelte 5 (runes) + SvelteKit con `adapter-static` |
| Estilos | Tailwind CSS 4 vía `@tailwindcss/vite` |
| Backend | Rust (edition 2024, tokio, serde) |
| Motor | yt-dlp + ffmpeg (del sistema o copia gestionada descargada por la app) |

Estado de navegación: **no hay rutas de SvelteKit en uso**. La app cambia de vista con un estado `view` en `src/routes/+page.svelte` (`"descargas" | "historial" | "ajustes"`). No agregues rutas nuevas sin discutirlo.

## Estructura

```
downlink/
├── src/                         # Frontend
│   ├── routes/
│   │   ├── +page.svelte         # Layout + switch de vistas (Descargas / Historial / Ajustes)
│   │   ├── +layout.svelte
│   │   └── +layout.ts           # ssr = false, prerender
│   ├── lib/
│   │   ├── api.ts               # Envoltorios tipados de invoke()/listen()/diálogos
│   │   ├── types.ts             # Tipos compartidos con el backend (camelCase)
│   │   ├── options.ts           # Presets, formatos, resumen de opciones
│   │   ├── utils.ts             # Formatos de duración/bytes/velocidad/ETA
│   │   ├── components/
│   │   │   ├── Sidebar.svelte       # Navegación + versión en el pie
│   │   │   ├── PreviewCard.svelte   # Metadatos del probe
│   │   │   ├── OptionsPanel.svelte  # Pestañas Video/Audio, presets, volumen
│   │   │   ├── PlaylistPanel.svelte # Selección de vídeos de una playlist
│   │   │   ├── QueueList.svelte     # Sección de cola + drag & drop entre pendientes
│   │   │   └── DownloadRow.svelte   # Fila de un trabajo (estados y acciones)
│   │   └── views/
│   │       ├── DownloadsView.svelte # URL, probe, opciones, cola, pegado/arrastre
│   │       ├── HistoryView.svelte   # Galería del historial, búsqueda, re-descarga
│   │       └── SettingsView.svelte  # Cookies, concurrencia, motor, probar conexión
│   └── app.css                  # Tailwind + base oscura
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Entry point
│   │   ├── lib.rs               # Builder, comandos Tauri, estado, workaround Wayland
│   │   ├── queue.rs             # Cola de trabajos (estados, concurrencia, pausa, orden)
│   │   ├── engine.rs            # Resolución de yt-dlp/ffmpeg y su origen (sistema/gestionado/sidecar)
│   │   ├── updater.rs           # Descarga y auto-update de yt-dlp y ffmpeg (GitHub releases)
│   │   ├── history.rs           # Historial persistido (history.json), dedupe por enlace
│   │   ├── settings.rs          # Settings persistidos, detección de navegadores, EngineInfo
│   │   └── ytdlp.rs             # Args, probe, parser de salida, spawn, errores, tests
│   ├── Cargo.toml
│   └── tauri.conf.json
└── .github/workflows/ci.yml     # CI: frontend + Rust (ubuntu + windows)
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
```

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

### La cola (`queue.rs`)

`Queue` es el estado gestionado por Tauri. Cada trabajo recorre
`queued → downloading → processing → done | error`, más `paused` y `cancelled`.

- **Concurrencia** (`Settings.concurrency`, 1–8, por defecto 2): `pump()` arranca
  pendientes hasta llenar los huecos y vuelve a llamar al terminar cada trabajo.
- **Pausa**: mata el proceso; `yt-dlp` reanuda el archivo `.part` al reanudar, así
  que **no** se empieza de cero (no pasar `--continue` a mano).
- **Orden**: el orden del vector `jobs` es el orden de despacho de los pendientes.
  `reorder_queue` lo cambia; la UI solo reordena los que están `queued`.

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

### Playlists

`probe` usa `--flat-playlist` (rápido, sin extraer cada vídeo) y devuelve
`kind: "playlist"` con `entries` (`id`, `title`, `url`, `duration`, `thumbnail`).
Las entradas de YouTube solo traen el id, así que `entry_url` construye
`https://www.youtube.com/watch?v=<id>` cuando la URL no es absoluta. La descarga
sigue siendo por vídeo suelto (`--no-playlist` en `build_download_args`).

Dos salvaguardas en `playlist_entries` / `probe_args`:

- **Deduplicación por `id`**: las radios/mixes de YouTube repiten vídeos. Con ids
  repetidos las claves del `{#each}` chocaban y rompían la vista.
- **Tope de radios** (`RADIO_PLAYLIST_LIMIT = 50`): `is_youtube_radio` detecta
  `list=RD…` / `start_radio=1` y añade `--playlist-end 50`; son listas
  autogeneradas que pueden no tener fin. `ProbeInfo.truncated` avisa a la UI.

La selección en el frontend es **por índice** (`Set<number>`), no por id, para no
depender de que los ids sean únicos.

### Historial (`history.rs`)

Al terminar con éxito un trabajo, `queue.rs` (`finish`) guarda un `HistoryEntry`
en `HistoryState`: `url` (identidad), `title`, `thumbnail`, `filePath`, `options`,
`sizeBytes` y `finishedAt` (millis). Se persiste en `history.json` dentro de
`app_config_dir()` (máx. 500 entradas, **dedupe por `url`**: re-descargar mueve la
entrada al frente en lugar de duplicarla). La miniatura llega desde el frontend vía
`start_download` (viaja en `JobView.thumbnail`). La vista `HistoryView.svelte` muestra
la galería, busca y re-descarga reutilizando las `options` guardadas.

### Esquema de argumentos

`build_download_args(settings, url, options)` arma todo. Los argumentos comunes (`common_args`) añaden cookies, runtime JS y EJS. `--ignore-config` **siempre** se pasa: se ignora el `yt-dlp.conf` del usuario a propósito.

`DownloadOptions` (`camelCase` en JSON):
`mode` (`"video" | "audio"`), `videoHeight`, `container`, `audioFormat`, `audioQuality`, `volumeDb`, `normalizeAudio`, `embedMetadata`, `embedThumbnail`, `clipStart`, `clipEnd`, `subtitles`, `subtitleLangs`, `subtitleAuto`, `embedSubtitles`.

- **Recorte**: `clipStart`/`clipEnd` (segundos, `f64`) → `--download-sections "*HH:MM:SS-HH:MM:SS"` (fin `inf` si falta). **No** se usa `--force-keyframes-at-cuts`: re-codifica con los códecs por defecto de ffmpeg (mpeg4) y rompería la preferencia H.264/AAC del MP4. El corte queda alineado a keyframes (puede variar unas décimas).
- **Subtítulos**: `subtitles` activa `--write-subs` (+ `--write-auto-subs` si `subtitleAuto`) y `--sub-langs` (por defecto `es`). Con `embedSubtitles` (solo vídeo) usa `--embed-subs`; si no, `--convert-subs srt` para dejar `.srt` al lado. En audio nunca se incrustan.
- Los campos nuevos llevan `#[serde(default)]` para que el `history.json` antiguo siga cargando.

### Motor (yt-dlp y ffmpeg)

`engine.rs` resuelve la ruta de cada binario y su **origen**, en este orden:

1. Variable de entorno (`DOWNLINK_YTDLP` / `DOWNLINK_FFMPEG`).
2. Copia **gestionada** por la app en `<app_data>/bin` (base para descargar/auto-actualizar).
3. **Sidecar** junto al ejecutable (mecanismo `externalBin` de Tauri).
4. `PATH` del sistema.

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
solo para **Linux y Windows** (x86_64 y arm64). Se verifica igual que yt-dlp
(SHA-256 contra `checksums.sha256` + ejecución de `-version`).

- **Desempaquetado en Rust**: `.tar.xz` con `lzma-rs` + `tar`, `.zip` con `zip`
  (feature `deflate`). **No** se depende de `tar`/`xz`/`unzip` del sistema. La
  descompresión corre en `spawn_blocking` (cientos de MB) y primero baja a un `.tar`
  temporal para no cargar todo en memoria.
- **Cuándo se instala**: `check_ffmpeg` decide con `ffmpeg_update_available`:
  - sin ffmpeg → hay que descargarlo;
  - copia **gestionada** → solo si la fecha de compilación remota (`updated_at` del
    asset) es más nueva que la del build instalado (se extrae de `ffmpeg -version`,
    p. ej. `N-127564-g3c92f003c7-20261015` → `20261015`);
  - ffmpeg del **sistema** o por `DOWNLINK_FFMPEG` → se respeta, no se toca.
- **Automático**: al arrancar solo se descarga **si falta** ffmpeg y
  `Settings.autoDownloadFfmpeg` está activo. **No** se refresca solo: cada build pesa
  ~150 MB y se regenera a diario, así que actualizar una copia existente es manual
  desde Ajustes.
- Los dos binarios se instalan juntos con `rename` para dejarlos siempre a la par.
- Test de integración opcional (no corre en CI): `DOWNLINK_FFMPEG_TEST_ARCHIVE=<archivo>
  cargo test -- --ignored extrae_binarios_reales_de_ffmpeg`.


## Convenciones de código

- **Español** en comentarios, docs, textos de UI y mensajes de error. Mensajes claros y accionables (dicen qué hacer, no solo qué falló).
- **Nada de `unwrap()`** en rutas que manejen entrada del usuario: usa `Result` + mensajes.
- Rust: `cargo fmt` y `clippy -D warnings` deben estar limpios. Tests unitarios junto al código (`#[cfg(test)] mod tests`).
- Frontend: Svelte 5 con **runes** (`$state`, `$derived`, `$props`, `$bindable`, `$state.snapshot`). No uses stores ni sintaxis de Svelte 4.
- Al enviar opciones al backend desde estado reactivo usa `$state.snapshot(options)` (los proxies no serializan bien).
- Commits: [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `chore:`, `docs:`).
- Estilo visual: tema oscuro tipo Linear, acento violeta `#8b5cf6` (violet-600).

## Cosas específicas del entorno y del motor

- **Wayland + NVIDIA**: `lib.rs` define `WEBKIT_DISABLE_DMABUF_RENDERER=1` al arrancar (Linux). Sin esto, WebKitGTK falla con `Error 71 (Protocol error)`. No lo quites.
- **YouTube anti-bot**: requiere cookies + runtime JS (node/deno) + solver EJS (`--remote-components ejs:github`, solo en dominios de YouTube). Los errores se traducen a mensajes que apuntan a Ajustes.
- **ffmpeg**: obligatorio para convertir formatos y aplicar filtros de audio.
- Variables de entorno útiles (override, sobre todo en tests): `DOWNLINK_YTDLP` y `DOWNLINK_FFMPEG` (rutas de los binarios), `DOWNLINK_JS_RUNTIME` (runtime forzado), `WEBKIT_DISABLE_DMABUF_RENDERER`.
- **Config y ajustes persistidos**: `settings.json` y `history.json` dentro de `app_config_dir()` (en Linux, `~/.config/<bundle-id>/`).
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

## CI

`.github/workflows/ci.yml` — dos jobs en push/PR a `main`:
- **frontend** (ubuntu): `npm ci` → `npm run check` → `npm run build`.
- **rust** (ubuntu + windows, matriz): `cargo fmt --check` → `cargo clippy -D warnings` → `cargo test` → `cargo check`.

Verifica el CI con: `gh run watch <run-id> --exit-status`.

## Estado y roadmap

Ver `README.md` → sección Roadmap (fuente de verdad del plan) y `CHANGELOG.md` (lo ya entregado).

Resumen:
- **v0.1 / v0.1.1** ✅ — cimientos, cookies del navegador, EJS, vista Ajustes.
- **v0.2** ✅ — selector de formatos, contenedores, formatos de audio, volumen/normalización, presets.
- **v0.3** ✅ — cola inteligente: concurrencia, pausa/reanudar, reordenar arrastrando, pegado múltiple.
- **v0.4** ✅ — playlists: detección, listado, selección y encolado.
- **v0.5** ✅ — historial con galería, búsqueda y re-descarga + persistencia local.
- **v0.6** ✅ — recorte de clips (`--download-sections`) y subtítulos (descarga e incrustado).
- **v0.7** 🚧 — madurez: detección de sidecars ✅ (v0.6.1), auto-update de yt-dlp ✅ (v0.7.0), descarga de ffmpeg ✅ (v0.7.1); pendiente tema claro, sidecars empaquetados, tray e instaladores.
- **v1.0** — Android (arm64); requiere empaquetar yt-dlp/ffmpeg por arquitectura.

## Al terminar un cambio

1. `cargo fmt` + `clippy -D warnings` + `cargo test` limpios.
2. `npm run check` y `npm run build` sin errores.
3. Actualiza `CHANGELOG.md` si es un cambio visible para el usuario.
4. Si toca arquitectura/comandos/convenciones, **actualiza este `AGENTS.md`**.
