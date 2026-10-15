# Changelog

Todo lo notable de Downlink se documenta aquí.

Formato basado en [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/),
y el proyecto adhiere a [Semantic Versioning](https://semver.org/lang/es/).

## [Unreleased]

### Added
- **UI base estilo Linear**: layout con sidebar, tema oscuro, acento violeta (Tailwind 4)
- **Análisis de enlaces**: pega un link (o Ctrl+V en cualquier parte) → preview con thumbnail, título, canal y duración
- **Descargas en tiempo real**: barra de progreso, velocidad y ETA vía eventos Tauri; cancelar en marcha
- Backend Rust: comandos `probe_url`, `start_download`, `cancel_download`; JobManager con tokio
- Scaffold inicial: Tauri 2 + Svelte 5 (SvelteKit, adapter-static)
- CI en GitHub Actions: svelte-check + build (frontend), fmt + clippy (Rust, Linux y Windows)
- README, CONTRIBUTING y estructura base del repo
