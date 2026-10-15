# Changelog

Todo lo notable de Downlink se documenta aquí.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/),
y el proyecto adhiere a [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

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
