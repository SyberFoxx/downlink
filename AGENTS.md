# AGENTS.md — Guía para agentes de IA

Instrucciones y contexto para agentes que trabajen en este repositorio. **Mantén este archivo actualizado al cambiar arquitectura, comandos o convenciones.**

## Qué es Downlink

App de escritorio (Tauri 2) para descargar video/audio de +1800 sitios con una UI moderna (Svelte 5). El motor de descarga es `yt-dlp`; `ffmpeg` hace conversión, remux, filtros de audio e incrustado de metadatos. Ambos se invocan como **procesos externos** (todavía no empaquetados como sidecars).

- Repo: `github.com/SyberFoxx/downlink` (privado)
- Versión actual: **0.2.0**
- Idioma del proyecto: **español** (comentarios, docs, textos de UI, mensajes de error). Los identificadores de código también están en español cuando es natural.

## Stack

| Capa | Tecnología |
|---|---|
| Shell nativo | Tauri 2 (Rust) |
| Frontend | Svelte 5 (runes) + SvelteKit con `adapter-static` |
| Estilos | Tailwind CSS 4 vía `@tailwindcss/vite` |
| Backend | Rust (edition 2024, tokio, serde) |
| Motor | yt-dlp + ffmpeg (binarios del sistema) |

Estado de navegación: **no hay rutas de SvelteKit en uso**. La app cambia de vista con un estado `view` en `src/routes/+page.svelte` (`"downloads" | "settings"`). No agregues rutas nuevas sin discutirlo.

## Estructura

```
downlink/
├── src/                         # Frontend
│   ├── routes/
│   │   ├── +page.svelte         # Layout + switch de vistas (Descargas / Ajustes)
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
│   │   │   └── DownloadRow.svelte   # Fila de progreso por trabajo
│   │   └── views/
│   │       ├── DownloadsView.svelte # URL, probe, opciones, lista de trabajos
│   │       └── SettingsView.svelte  # Cookies, estado del motor, probar conexión
│   └── app.css                  # Tailwind + base oscura
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Entry point
│   │   ├── lib.rs               # Builder, comandos Tauri, estado (jobs), workaround Wayland
│   │   ├── settings.rs          # Settings persistidos, detección de navegadores, EngineInfo
│   │   └── ytdlp.rs             # Args de yt-dlp, probe, descarga, errores, tests
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
| `probe_url` | `url` | `ProbeInfo` (metadatos + `heights`) |
| `start_download` | `url`, `options` | `job_id: u64` |
| `cancel_download` | `id` | — (emite evento `cancelled`) |
| `get_settings` / `save_settings` | `settings` | `Settings` |
| `detect_browsers` | — | `BrowserInfo[]` |
| `engine_info` | — | `EngineInfo` (yt-dlp, runtime JS, ffmpeg) |

### Evento de progreso

Canal único `download-progress` (`DOWNLOAD_EVENT`), payload `ProgressPayload`:

```jsonc
{
  "jobId": 1, "percent": 42.5, "downloadedBytes": 123, "totalBytes": 456,
  "speedBps": 789, "etaSecs": 10,
  "status": "downloading | done | error | cancelled",
  "filePath": "/home/…/video.mp4", "message": null
}
```

Protocolo de línea sobre stdout de yt-dlp (en `ytdlp.rs`):
- `__DL__<descargado>|<total>|<velocidad>|<eta>|<porcentaje>` → progreso
- `__FILE__<ruta>` → ruta final del archivo

### Esquema de argumentos

`build_download_args(settings, url, options)` arma todo. Los argumentos comunes (`common_args`) añaden cookies, runtime JS y EJS. `--ignore-config` **siempre** se pasa: se ignora el `yt-dlp.conf` del usuario a propósito.

`DownloadOptions` (`camelCase` en JSON):
`mode` (`"video" | "audio"`), `videoHeight`, `container`, `audioFormat`, `audioQuality`, `volumeDb`, `normalizeAudio`, `embedMetadata`, `embedThumbnail`.

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
- Variables de entorno útiles (override, sobre todo en tests): `DOWNLINK_YTDLP` (ruta del binario), `DOWNLINK_JS_RUNTIME` (runtime forzado), `WEBKIT_DISABLE_DMABUF_RENDERER`.
- **Config y ajustes persistidos**: `settings.json` dentro de `app_config_dir()` (en Linux, `~/.config/<bundle-id>/`).
- Los archivos se descargan en `download_dir()` con patrón `%(title)s [%(id)s].%(ext)s`.

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
- **rust** (ubuntu + windows, matriz): `cargo fmt --check` → `cargo clippy -D warnings` → `cargo check`.

Verifica el CI con: `gh run watch <run-id> --exit-status`.

## Estado y roadmap

Ver `README.md` → sección Roadmap (fuente de verdad del plan) y `CHANGELOG.md` (lo ya entregado).

Resumen:
- **v0.1 / v0.1.1** ✅ — cimientos, cookies del navegador, EJS, vista Ajustes.
- **v0.2** 🚧 — selector de formatos, contenedores, formatos de audio, volumen/normalización, presets. **Pendiente**: cola con drag & drop y concurrencia.
- **v0.3** — playlists, recorte de clips (`--download-sections`), subtítulos, historial.
- **v0.4** — sidecars (empaquetar yt-dlp/ffmpeg) + auto-update, system tray, instaladores.
- **v1.0** — Android (arm64); requiere empaquetar yt-dlp/ffmpeg por arquitectura.

## Al terminar un cambio

1. `cargo fmt` + `clippy -D warnings` + `cargo test` limpios.
2. `npm run check` y `npm run build` sin errores.
3. Actualiza `CHANGELOG.md` si es un cambio visible para el usuario.
4. Si toca arquitectura/comandos/convenciones, **actualiza este `AGENTS.md`**.
