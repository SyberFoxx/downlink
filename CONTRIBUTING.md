# Contribuir en Downlink

Guía corta para mantener el proyecto ordenado.

## Flujo de trabajo

1. `main` siempre compila y tiene el CI verde.
2. Todo cambio vive en una rama: `feature/*`, `fix/*` o `chore/*`.
3. Abrir un PR contra `main`; el CI (fmt, clippy, svelte-check) debe pasar.
4. Merge con rebase o squash (sin merge commits).

## Commits

[Conventional Commits](https://www.conventionalcommits.org/):

```
feat: barra de progreso en tiempo real
fix: crash al pegar link de playlist vacía
chore: bump tauri-cli a 2.9
docs: sección de requisitos en el README
```

## Antes de abrir el PR

```bash
npm run check            # tipos Svelte/TS
cd src-tauri && cargo fmt --all && cargo clippy -- -D warnings
```

## Estructura

- `src/` — frontend Svelte 5 (SvelteKit, adapter-static). Componentes en `src/lib/components/`.
- `src-tauri/src/` — backend Rust: comandos Tauri, JobManager (cola de descargas), SidecarManager (yt-dlp/ffmpeg).

## Reglas de oro

- El frontend **nunca** habla directamente con los sidecars: todo pasa por comandos Tauri en Rust.
- Los eventos de progreso salen de Rust con `app.emit(...)`; el frontend solo escucha.
- Nada de `unwrap()` en código que pueda fallar con entradas del usuario — usar `Result` y mensajes claros.
