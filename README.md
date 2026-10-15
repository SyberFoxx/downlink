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
| Recorte de clips | ✅ | ❌ | ✅ |
| Auto-update de yt-dlp | ✅ | ❌ | ✅ |

## Características

- 🎯 **Pega el link y listo** — detecta la plataforma, muestra preview (thumbnail, título, duración, canal)
- 🍪 **Cookies del navegador** — usa tu sesión de Brave/Firefox/Chrome para YouTube y contenido restringido
- 🧠 **Desafíos JS resueltos** — detecta el runtime (node/deno) y usa el solver EJS oficial de yt-dlp
- 🎞️ **Selector de formatos visual** — tabs Video / Audio / Personalizado, sin IDs crípticos
- ⚡ **Presets** — "MP3 320k", "1080p MP4", "máxima calidad" en un click
- 📋 **Cola inteligente** — drag & drop, pausar item individual, descargas concurrentes configurables
- ✂️ **Recortar clips** — define inicio/fin antes de descargar, sin transcodificar de más
- 🏷️ **Metadatos automáticos** — título, artista e imagen incrustados en archivos de audio
- 📜 **Historial con galería** — búsqueda, miniaturas, re-descarga con un click
- 🔄 **Auto-update de yt-dlp** — YouTube cambia cada dos semanas; Downlink se mantiene al día
- 🌙 **Tema oscuro/claro**

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

Los binarios de `yt-dlp` y `ffmpeg` se empaquetan junto a la app (aún no — ver [roadmap](#roadmap)).

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
│   │   └── lib.rs        # Setup de Tauri, comandos, gestión de jobs
│   ├── Cargo.toml
│   └── tauri.conf.json   # Configuración de ventana, bundle, etc.
└── .github/workflows/    # CI (Linux + Windows)
```

## Roadmap

### v0.1 — Cimientos 🚧
- [x] Scaffold Tauri 2 + Svelte 5
- [ ] UI base: layout con sidebar, tema oscuro/claro
- [ ] Pegar link → probe de metadatos → preview card
- [ ] Descarga simple con barra de progreso en tiempo real

### v0.2 — Cola y formatos
- [ ] Cola con drag & drop, pausa/cancelar, concurrencia
- [ ] Selector de formatos visual (video/audio/personalizado)
- [ ] Presets configurables

### v0.3 — Extras
- [ ] Playlists y canales (expansión y selección de items)
- [ ] Recorte de clips (`--download-sections`)
- [ ] Subtítulos (descarga e incrustado)
- [ ] Historial con galería y búsqueda
- [ ] Metadatos incrustados en audio

### v0.4 — Madurez
- [ ] Empaquetado de yt-dlp/ffmpeg como sidecars + auto-update
- [x] ~~Importación de cookies para contenido privado~~ (hecho en v0.1.1)
- [ ] System tray con progreso + notificaciones
- [ ] Instaladores (deb/rpm/AppImage, Windows MSI/NSIS)

### v1.0 — Android 📱
- [ ] Port a Tauri 2 mobile (arm64)
- [ ] UI adaptada a móvil (gestos, bottom nav)

## Contribuir

Ver [CONTRIBUTING.md](CONTRIBUTING.md). Idea general: `main` siempre estable, ramas `feature/*`, conventional commits, CI verde antes de mergear.

## Licencia

[MIT](LICENSE) © Ramses Sotomayor
