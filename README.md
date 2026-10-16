<div align="center">

# ⬇️ Downlink

**Descargador de videos y audio multiplataforma con interfaz moderna**

[![CI](https://github.com/SyberFoxx/downlink/actions/workflows/ci.yml/badge.svg)](https://github.com/SyberFoxx/downlink/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blueviolet?logo=tauri&logoColor=white)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-orange?logo=svelte&logoColor=white)](https://svelte.dev)

</div>

---

## ¿Qué es Downlink?

Downlink es una aplicación de escritorio (y próximamente Android) para descargar videos y audio de **+1800 sitios web** — YouTube, TikTok, Instagram, X/Twitter, Twitch, Reddit y muchos más — con una interfaz limpia, rápida y pensada para el usuario final.

Bajo el capó usa [yt-dlp](https://github.com/yt-dlp/yt-dlp) y `ffmpeg` como *sidecars*, lo que significa soporte de sitios siempre actualizado sin sacrificar una experiencia de usuario nativa.

### ¿Por qué Downlink y no otra cosa?

| | Downlink | MeTube | YtDlnis |
|---|---|---|---|
| Interfaz moderna (Svelte 5) | ✅ | ❌ (web básica) | ⚠️ tosca |
| Escritorio (Linux/Windows) | ✅ | ⚠️ vía navegador | ❌ |
| Android | 🚧 planeado | ❌ | ✅ |
| Selector de formatos visual | ✅ | ❌ | ⚠️ |
| Presets de un click | ✅ | ❌ | ✅ |
| Cola con drag & drop | ✅ | ❌ | ❌ |
| Historial con galería | ✅ | ⚠️ básico | ✅ |
| Recorte de clips | ✅ | ❌ | ✅ |
| Subtítulos (descarga e incrustado) | ✅ | ❌ | ✅ |
| Auto-update de yt-dlp | ✅ | ❌ | ✅ |

## Características

### Descarga y cola
- 🎯 **Pega el link y listo** — detecta la plataforma, muestra preview (thumbnail, título, duración, canal)
- 📋 **Cola inteligente** — varias descargas a la vez, con pausa y reanudación individual
- 🧲 **Reordena arrastrando** — el orden en que arrastras los pendientes es el orden en que se descargan
- 🖱️ **Arrastra enlaces a la ventana** — o pega varios enlaces de golpe con `Ctrl+V`
- 🎬 **Playlists y canales** — lista todos los vídeos, elige cuáles y van a la cola de golpe
- 🔢 **Concurrencia configurable** — de 1 a 8 descargas simultáneas según tu conexión
- 📂 **Mostrar en la carpeta** — un click para abrir el archivo terminado

### Historial
- 🖼️ **Galería de descargas** — cada descarga completada se guarda con su miniatura, fecha, tamaño y opciones
- 🔍 **Búsqueda instantánea** — filtra por título o enlace entre todas tus descargas
- ♻️ **Re-descarga en un click** — repite una descarga con las mismas opciones de la primera vez
- 💾 **Persistencia local** — el historial vive en tu equipo y sobrevive al reinicio

### Formatos y audio
- 🎞️ **Selector de formatos visual** — resoluciones reales del enlace, contenedores MP4/MKV/WEBM
- 🎵 **Todos los formatos de audio** — MP3, M4A, OGG, OPUS, FLAC, WAV u original sin recomprimir
- 🔊 **Control de volumen** — sube o baja ±20 dB y normaliza el audio con ffmpeg
- ⚡ **Presets de un click** — Máxima calidad, Video 1080p, MP3 320k, FLAC, Audio original
- 🏷️ **Metadatos automáticos** — título, artista e imagen incrustados en archivos de audio

### Recorte y subtítulos
- ✂️ **Recorta un fragmento** — indica inicio y fin (en `mm:ss` o `h:mm:ss`) y descarga solo esa parte
- 💬 **Subtítulos** — descárgalos en los idiomas que quieras (`es,en`…), incluidos los automáticos
- 🧩 **Incrustados o como archivo** — embébelos en el vídeo (MP4/MKV) o déjalos como `.srt` sueltos

### Motor y compatibilidad
- 🧩 **Detección del motor** — muestra de dónde sale cada binario (sistema, gestionado por Downlink o incluido con la app)
- ⬆️ **Auto-update de yt-dlp** — descarga su propia copia desde los releases oficiales, la verifica (SHA-256 + ejecución) y la mantiene al día sola
- 🍪 **Cookies del navegador** — usa tu sesión de Brave/Firefox/Chrome para YouTube y contenido restringido
- 🧠 **Desafíos JS resueltos** — detecta el runtime (node/deno) y usa el solver EJS oficial de yt-dlp
- 🔧 **Estado del motor** — yt-dlp, runtime JS y ffmpeg detectados y probados desde Ajustes

En el [roadmap](#roadmap): tema claro, auto-update de ffmpeg, empaquetado de yt-dlp/ffmpeg, system tray e instaladores.

## Tech Stack

- **[Tauri 2](https://tauri.app)** — shell nativo, binario ~5 MB, multiplataforma (Linux, Windows, 🚧 macOS, 🚧 Android)
- **[Svelte 5](https://svelte.dev)** + **[SvelteKit](https://kit.svelte.dev)** — UI reactiva, sin Virtual DOM, arranque <1s
- **[Rust](https://www.rust-lang.org)** — backend: gestión de cola, procesos sidecar, eventos en tiempo real
- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** — motor de descarga (sidecar)
- **[ffmpeg](https://ffmpeg.org)** — conversión, remux, incrustación de metadatos (sidecar)

## Requisitos

Para desarrollar:

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) ≥ 20
- Dependencias de sistema de [Tauri para tu SO](https://tauri.app/start/prerequisites/)

En Fedora:

```bash
sudo dnf install webkit2gtk4.1-devel libsoup3-devel librsvg2-devel \
  openssl-devel libayatana-appindicator-gtk3-devel gcc
```

En Debian/Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Downlink descarga su propia copia de `yt-dlp` en la primera actualización (no necesitas instalarlo). `ffmpeg` y un runtime JS (`node`/`deno`) sí debes tenerlos en el sistema por ahora — ver [roadmap](#roadmap).

## Solución de problemas

**«YouTube pidió verificación anti-bot» / «Sign in to confirm you're not a bot»**
YouTube bloquea por IP cuando hay muchas peticiones o sesión no válida. Solución:
1. Abre **Ajustes** en Downlink → Cookies → **Desde el navegador** → elige tu navegador (con sesión iniciada en YouTube) → Guardar.
2. Pulsa **Probar conexión** para verificar.
3. Si sigue fallando, cierra el navegador (para que la base de cookies no esté bloqueada) e intenta de nuevo.

**«n challenge solving failed»**
Falta un runtime JS: instala `node` (`sudo dnf install nodejs`) o `deno`. Downlink lo detecta automáticamente.

## Desarrollo

```bash
git clone https://github.com/SyberFoxx/downlink.git
cd downlink
npm install

# App en modo desarrollo (hot-reload)
npm run tauri dev

# Chequeos
npm run check        # TypeScript + Svelte
npm run tauri build  # Build de producción
```

## Estructura del proyecto

```
downlink/
├── src/                  # Frontend Svelte 5 (SvelteKit, adapter-static)
├── src-tauri/            # Backend Rust
│   ├── src/
│   │   ├── main.rs       # Entry point
│   │   ├── lib.rs        # Setup de Tauri, comandos, gestión de jobs
│   │   ├── queue.rs      # Cola de descargas (estados, concurrencia, orden)
│   │   ├── engine.rs     # Resolución de yt-dlp/ffmpeg y su origen
│   │   ├── updater.rs    # Descarga y auto-update de yt-dlp (GitHub releases)
│   │   ├── history.rs    # Historial persistido (history.json)
│   │   ├── settings.rs   # Ajustes persistidos y detección de navegadores
│   │   └── ytdlp.rs      # Argumentos, probe y parser de yt-dlp
│   ├── Cargo.toml
│   └── tauri.conf.json   # Configuración de ventana, bundle, etc.
└── .github/workflows/    # CI (Linux + Windows)
```

## Roadmap

### v0.1 — Cimientos ✅
- [x] Scaffold Tauri 2 + Svelte 5
- [x] UI base: layout con sidebar y tema oscuro
- [x] Pegar link → probe de metadatos → preview card
- [x] Descarga simple con barra de progreso en tiempo real
- [x] Cookies del navegador + solver EJS + vista Ajustes (v0.1.1)

### v0.2 — Formatos y audio ✅
- [x] Selector de formatos visual (video/audio, resoluciones reales)
- [x] Contenedores MP4/MKV/WEBM + códecs compatibles en MP4
- [x] Formatos de audio (MP3/M4A/OGG/OPUS/FLAC/WAV/original)
- [x] Control de volumen y normalización (ffmpeg)
- [x] Presets configurables

### v0.3 — Cola inteligente ✅
- [x] Cola con varios trabajos y estados (en cola, descargando, procesando, pausado…)
- [x] Pausar y reanudar sin empezar de cero
- [x] Reordenar los pendientes arrastrando
- [x] Concurrencia configurable (1–8 descargas simultáneas)
- [x] Pegar varios enlaces a la vez / arrastrarlos a la ventana

### v0.4 — Playlists ✅
- [x] Detección de playlists y canales al analizar el enlace
- [x] Lista de vídeos con título y duración
- [x] Selección individual, todos o ninguno
- [x] Encolar los vídeos elegidos de golpe

### v0.5 — Historial ✅
- [x] Historial con galería, búsqueda y re-descarga
- [x] Persistencia local de lo descargado

### v0.6 — Recorte y subtítulos ✅
- [x] Recorte de clips (`--download-sections`)
- [x] Subtítulos (descarga e incrustado)

### v0.7 — Madurez
- [x] Detección del origen del motor (sidecar / gestionado / sistema)
- [x] Descarga y auto-update de **yt-dlp**
- [ ] Descarga y auto-update de ffmpeg
- [ ] Tema claro
- [ ] System tray con progreso + notificaciones
- [ ] Instaladores (deb/rpm/AppImage, Windows MSI/NSIS)

### v1.0 — Android 📱
- [ ] Port a Tauri 2 mobile (arm64)
- [ ] UI adaptada a móvil (gestos, bottom nav)
- [ ] yt-dlp/ffmpeg empaquetados por arquitectura (arm64)

## Contribuir

Ver [CONTRIBUTING.md](CONTRIBUTING.md). Idea general: `main` siempre estable, ramas `feature/*`, conventional commits, CI verde antes de mergear.

## Licencia

[MIT](LICENSE) © Ramses Sotomayor
