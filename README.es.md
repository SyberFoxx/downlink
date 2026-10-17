<div align="center">

<img src="static/brand/icon.svg" alt="Logotipo de Downlink" width="84" height="84" />

# Downlink

**Descargador de vídeo y audio para escritorio, con una interfaz moderna**

[![CI](https://github.com/SyberFoxx/downlink/actions/workflows/ci.yml/badge.svg)](https://github.com/SyberFoxx/downlink/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tauri](https://img.shields.io/badge/Tauri-2.x-blueviolet?logo=tauri&logoColor=white)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Svelte-5-orange?logo=svelte&logoColor=white)](https://svelte.dev)

[English](README.md) | **Español**

</div>

---

## ¿Qué es Downlink?

Downlink es una aplicación de escritorio (y, próximamente, Android) para descargar vídeo y audio de **más de 1800 sitios web** —YouTube, TikTok, Instagram, X/Twitter, Twitch, Reddit y muchos más— con una interfaz limpia, rápida y pensada para el usuario final.

Bajo el capó usa [yt-dlp](https://github.com/yt-dlp/yt-dlp) y `ffmpeg` como procesos externos: los del sistema o, si no están, copias que la propia aplicación descarga y mantiene al día. Eso da un soporte de sitios siempre actualizado sin renunciar a una experiencia nativa.

### ¿Por qué Downlink y no otra opción?

| | Downlink | MeTube | YTDLnis |
|---|---|---|---|
| Interfaz moderna (Svelte 5) | ✅ | ❌ (web básica) | ⚠️ tosca |
| Escritorio (Linux/Windows) | ✅ | ⚠️ vía navegador | ❌ |
| Android | 🚧 planeado | ❌ | ✅ |
| Selector de formatos visual | ✅ | ❌ | ⚠️ |
| Presets de un clic | ✅ | ❌ | ✅ |
| Cola con arrastrar y soltar | ✅ | ❌ | ❌ |
| Historial con galería | ✅ | ⚠️ básico | ✅ |
| Recorte de clips | ✅ | ❌ | ✅ |
| Subtítulos (descarga e incrustado) | ✅ | ❌ | ✅ |
| Motor y runtime JS automáticos | ✅ | ❌ | ✅ |

## Características

### Descarga y cola
- 🎯 **Pega el enlace y listo** — detecta la plataforma y muestra una vista previa (miniatura, título, duración y canal)
- 📋 **Cola inteligente** — varias descargas a la vez, con pausa y reanudación individual
- 🧲 **Reordena arrastrando** — el orden en que colocas los pendientes es el orden en que se descargan
- 🖱️ **Arrastra enlaces a la ventana** — o pega varios enlaces de golpe con `Ctrl+V`
- 🎬 **Playlists y canales** — lista todos los vídeos, elige cuáles quieres y van a la cola de una vez
- 🔢 **Concurrencia configurable** — de 1 a 8 descargas simultáneas según tu conexión
- 📂 **Mostrar en la carpeta** — un clic para abrir el archivo terminado

### Formatos y audio
- 🎞️ **Selector de formatos visual** — resoluciones reales del enlace y contenedores MP4/MKV/WEBM
- 🎵 **Todos los formatos de audio** — MP3, M4A, OGG, OPUS, FLAC, WAV u original sin recomprimir
- 🔊 **Control de volumen** — sube o baja ±20 dB y normaliza el audio con `ffmpeg`
- ⚡ **Presets de un clic** — máxima calidad, vídeo 1080p, MP3 320k, FLAC o audio original
- 🏷️ **Metadatos automáticos** — título, artista e imagen incrustados en los archivos de audio

### Recorte y subtítulos
- ✂️ **Recorta un fragmento** — indica inicio y fin (en `mm:ss` o `h:mm:ss`) y descarga solo esa parte
- 💬 **Subtítulos** — descárgalos en los idiomas que quieras (`es,en`…), automáticos incluidos
- 🧩 **Incrustados o como archivo** — embébelos en el vídeo (MP4/MKV) o déjalos como `.srt` sueltos

### Historial
- 🖼️ **Galería de descargas** — cada descarga completada guarda su miniatura, fecha, tamaño y opciones
- 🔍 **Búsqueda instantánea** — filtra por título o enlace entre todas tus descargas
- ♻️ **Re-descarga en un clic** — repite una descarga con las mismas opciones de la primera vez
- 💾 **Persistencia local** — el historial vive en tu equipo y sobrevive al reinicio

### Escritorio
- 📂 **Carpeta de descarga configurable** — elige dónde guardar los archivos (se crea si no existe)
- 🛡️ **SponsorBlock** — quita o marca los segmentos promocionales de YouTube (patrocinio, intro, outro, relleno…)
- 💾 **Cola que persiste** — los pendientes se guardan y se recuperan al abrir la app; yt-dlp retoma el archivo parcial
- 🔔 **Notificaciones** — aviso del sistema al terminar o fallar, solo si la ventana no está delante
- 📌 **Bandeja del sistema** — menú con «Mostrar» y «Salir», estado de la cola y opción de cerrar a la bandeja
- 🔄 **Reintentar** — los trabajos fallidos o cancelados se reencolan con las mismas opciones

### Motor y compatibilidad
- 🧩 **Detección del motor** — muestra de dónde sale cada binario (del sistema, gestionado por Downlink o incluido con la app)
- ⬆️ **Auto-actualización de yt-dlp** — descarga su propia copia desde los *releases* oficiales, la verifica (SHA-256 y ejecución real) y la mantiene al día
- 📥 **ffmpeg sin complicaciones** — si no lo tienes, Downlink descarga su propia copia (con `ffprobe`), la verifica igual que yt-dlp y la usa sola; si ya está en el sistema, se respeta
- 🧠 **Runtime JS automático** — si no tienes Node.js, Deno ni Bun, Downlink descarga su propio intérprete (quickjs, ~2,5 MB) para que yt-dlp resuelva los desafíos de YouTube; ya no hay que instalar nada a mano
- 🍪 **Cookies del navegador** — usa tu sesión de Brave, Firefox, Chrome y compañía para YouTube y contenido restringido
- 🔧 **Estado del motor** — yt-dlp, runtime JS y ffmpeg detectados y probados desde Ajustes

### Apariencia
- 🎨 **Temas** — 10 incluidos (Oscuro, Claro, Catppuccin, Nord, Dracula, Gruvbox, Tokyo Night, Rosé Pine, Solarized…), aplicados al instante
- 🖌️ **Color de acento** — elige el color de botones, foco y progreso, o deja el del tema
- 🖼️ **Imagen de fondo** — pon tu fondo de pantalla por detrás de los paneles, con intensidad y desenfoque ajustables
- 🐧 **Se integra con tu escritorio** — el modo automático sigue el claro/oscuro del sistema y adopta la paleta de **pywal/wallust** (Hyprland y demás *ricing*)

## Instalar

Los instaladores de cada versión se publican solos en [GitHub Releases](https://github.com/SyberFoxx/downlink/releases):

| Plataforma | Formato |
|---|---|
| Debian / Ubuntu | `.deb` → `sudo apt install ./Downlink_*.deb` |
| Fedora / openSUSE | `.rpm` → `sudo dnf install ./Downlink-*.rpm` (o `sudo zypper install ./Downlink-*.rpm`) |
| Cualquier distro Linux | `AppImage` → `chmod +x Downlink_*.AppImage && ./Downlink_*.AppImage` |
| Windows 10 / 11 | `.msi` o `.exe` (NSIS) — instalar y listo |

macOS todavía no tiene paquete propio (está en el [roadmap](#roadmap)); por ahora se ejecuta desde el código con `npm run tauri dev`.

No necesitas instalar yt-dlp, ffmpeg ni un runtime JS: si no los encuentra en el sistema, Downlink descarga y mantiene sus propias copias.

### Empaquetar en local

```bash
npm run tauri build                            # deb + rpm + AppImage (Linux) / MSI + NSIS (Windows)
npm run tauri build -- --bundles deb,rpm       # solo los formatos que quieras
```

Los instaladores aparecen en `src-tauri/target/release/bundle/`. En Linux hacen falta los paquetes de desarrollo (`libayatana-appindicator3-dev` en Debian/Ubuntu, `libayatana-appindicator-gtk3-devel` en Fedora) y, para el AppImage, `libfuse2` y `patchelf`.

## Stack

- **[Tauri 2](https://tauri.app)** — shell nativo y binario ligero para Linux y Windows (Android en camino)
- **[Svelte 5](https://svelte.dev)** + **[SvelteKit](https://kit.svelte.dev)** — interfaz reactiva, sin DOM virtual
- **[Rust](https://www.rust-lang.org)** — backend: cola de descargas, procesos externos y eventos en tiempo real
- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** — motor de descarga (copia gestionada por la app o del sistema)
- **[ffmpeg](https://ffmpeg.org)** — conversión, remux e incrustación de metadatos
- **[quickjs](https://github.com/quickjs-ng/quickjs)** — runtime JS ligero para los desafíos de YouTube (opcional si ya usas Node.js, Deno o Bun)

## Requisitos

Para desarrollar:

- [Rust](https://rustup.rs) (stable)
- [Node.js](https://nodejs.org) (LTS, 20 o superior)
- Las dependencias de sistema de [Tauri para tu SO](https://tauri.app/start/prerequisites/)

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

Para usar la aplicación **no** hace falta instalar `yt-dlp`, `ffmpeg` ni un runtime JS: Downlink descarga sus propias copias si faltan.

## Solución de problemas

**«YouTube pidió verificación anti-bot» / «Sign in to confirm you're not a bot»**

YouTube bloquea por IP cuando hay muchas peticiones o la sesión no es válida. Solución:

1. Abre **Ajustes** → Cookies → **Desde el navegador** y elige tu navegador (con la sesión de YouTube iniciada).
2. Pulsa **Probar conexión** para verificar.
3. Si sigue fallando, cierra el navegador (para que su base de cookies no esté bloqueada) e inténtalo de nuevo.

**«Could not copy Chrome cookie database» (Windows)**

Windows **bloquea la base de cookies del navegador** mientras está abierto, así que yt-dlp no puede leerla — en Linux suele funcionar incluso con el navegador abierto, porque los bloqueos de Linux son solo de cortesía. Dice «Chrome» aunque uses Brave o Edge: es la palabra que trae yt-dlp, no un error tuyo.

1. Cierra el navegador por completo: sácalo de la bandeja y comprueba en el Administrador de tareas que no queden procesos `brave.exe`/`chrome.exe`.
2. Pulsa **Probar conexión** otra vez.
3. Si sigue fallando, exporta las cookies a un archivo `.txt` (p. ej. con *Get cookies.txt LOCALLY*) y cambia **Ajustes → Cookies → Desde archivo**.

Detalles: [yt-dlp#7271](https://github.com/yt-dlp/yt-dlp/issues/7271).

**«n challenge solving failed»**

Falta un runtime JS. Downlink lo descarga solo en la primera ejecución; si no ocurre, ve a **Ajustes → Motor → runtime JS** y pulsa **Descargar**. También puedes instalar `node` o `deno` tú mismo si lo prefieres.

## Desarrollo

```bash
git clone https://github.com/SyberFoxx/downlink.git
cd downlink
npm install

# Aplicación en modo desarrollo (con recarga en caliente)
npm run tauri dev
```

Antes de enviar cambios, estas comprobaciones deben quedar limpias:

```bash
npm run check                                            # tipados de Svelte y TypeScript
npm run build                                            # build del frontend
cd src-tauri && cargo fmt --all                          # formato de Rust
cd src-tauri && cargo clippy --all-targets -- -D warnings # lints
cd src-tauri && cargo test                               # tests unitarios
```

## Estructura del proyecto

```
downlink/
├── src/                     # Frontend Svelte 5 (SvelteKit con adapter-static)
│   ├── routes/+page.svelte  # Layout y cambio de vista (Descargas/Historial/Apariencia/Ajustes)
│   └── lib/                 # Componentes, vistas, estado reactivo y envoltorios de la API
├── src-tauri/               # Backend Rust
│   ├── src/
│   │   ├── main.rs          # Punto de entrada
│   │   ├── lib.rs           # Setup de Tauri, comandos y estado global
│   │   ├── queue.rs         # Cola de descargas (estados, concurrencia, orden, pausa)
│   │   ├── queue_store.rs   # Persistencia de la cola (queue.json)
│   │   ├── engine.rs        # Resolución de yt-dlp/ffmpeg/quickjs y su origen
│   │   ├── updater.rs       # Descarga y auto-actualización de los motores (GitHub Releases)
│   │   ├── history.rs       # Historial persistido (history.json)
│   │   ├── settings.rs      # Ajustes persistidos y detección de navegadores
│   │   ├── theme.rs         # Imagen de fondo y paleta de pywal/wallust
│   │   ├── tray.rs          # Icono de bandeja del sistema
│   │   └── ytdlp.rs         # Argumentos, análisis y salida de yt-dlp
│   ├── Cargo.toml
│   └── tauri.conf.json      # Configuración de ventana, bundle e identificador
├── docs/                    # Documentación adicional (plan de Android)
└── .github/workflows/       # CI (Linux + Windows) y publicación de releases
```

## Roadmap

### v0.1 — Cimientos ✅
- [x] Scaffold de Tauri 2 + Svelte 5
- [x] Interfaz base: layout con barra lateral y tema oscuro
- [x] Pegar enlace → análisis de metadatos → tarjeta de vista previa
- [x] Descarga con barra de progreso en tiempo real
- [x] Cookies del navegador, solver EJS y vista de Ajustes (v0.1.1)

### v0.2 — Formatos y audio ✅
- [x] Selector de formatos visual (vídeo/audio, resoluciones reales)
- [x] Contenedores MP4/MKV/WEBM con códecs compatibles en MP4
- [x] Formatos de audio (MP3/M4A/OGG/OPUS/FLAC/WAV/original)
- [x] Control de volumen y normalización con `ffmpeg`
- [x] Presets configurables

### v0.3 — Cola inteligente ✅
- [x] Varios trabajos con estados (en cola, descargando, procesando, pausado…)
- [x] Pausar y reanudar sin empezar de cero
- [x] Reordenar los pendientes arrastrando
- [x] Concurrencia configurable (1–8 descargas simultáneas)
- [x] Pegar o arrastrar varios enlaces a la vez

### v0.4 — Playlists ✅
- [x] Detección de playlists y canales al analizar el enlace
- [x] Lista de vídeos con título y duración
- [x] Selección individual, de todos o de ninguno
- [x] Encolar los vídeos elegidos de una vez

### v0.5 — Historial ✅
- [x] Historial con galería, búsqueda y re-descarga
- [x] Persistencia local de lo descargado

### v0.6 — Recorte y subtítulos ✅
- [x] Recorte de clips (`--download-sections`)
- [x] Subtítulos (descarga e incrustado)

### v0.7 — Motor y madurez ✅
- [x] Detección del origen del motor (gestionado, sidecar o del sistema)
- [x] Auto-actualización de yt-dlp (verificación SHA-256 y ejecución real)
- [x] Descarga de ffmpeg (Linux/Windows) cuando falta
- [ ] Empaquetar yt-dlp y ffmpeg como *sidecars* dentro del instalador

### v0.8 — Temas y apariencia ✅
- [x] Sistema de tokens de diseño para repintar la app sin recompilar
- [x] 10 temas, modo automático claro/oscuro y paleta de pywal/wallust
- [x] Color de acento personalizable e imagen de fondo con intensidad y desenfoque
- [x] Vista de Apariencia

### v0.9 — Escritorio ✅
- [x] Carpeta de descarga configurable
- [x] SponsorBlock: quitar o marcar segmentos promocionales por categoría
- [x] Cola persistente entre sesiones (`queue.json`)
- [x] Notificaciones al terminar (solo si la ventana no está delante)
- [x] Bandeja del sistema con estado de la cola y cierre a la bandeja
- [x] Reintentar trabajos fallidos o cancelados y avisos globales (toasts)
- [x] Pegar o arrastrar enlaces desde cualquier vista
- [x] Marca propia: logotipo e iconos de la aplicación

### v0.10 — Distribución y robustez ✅
- [x] Instaladores oficiales `.deb`, `.rpm`, AppImage, `.msi` y NSIS, publicados en GitHub Releases
- [x] Corrección del arranque de `ffmpeg` y `node` dentro del AppImage (v0.10.1)
- [x] Arreglos en Windows: detección de navegadores, sin ventanas de CMD y runtime JS automático (v0.10.2)

### v1.0 — Android 📱
- [ ] Port a Tauri 2 móvil (arm64)
- [ ] Interfaz adaptada a móvil (gestos, navegación inferior, áreas seguras)
- [ ] yt-dlp y ffmpeg empaquetados por arquitectura (arm64)

Plan detallado (bloqueantes, adaptaciones y fases): **[docs/android-v1.md](docs/android-v1.md)**.

> iOS queda fuera de alcance: la app lanza `yt-dlp` y `ffmpeg` como procesos externos, y iOS no permite crear procesos hijos.

Consulta el [CHANGELOG](CHANGELOG.md) para el detalle de cada versión.

## Contribuir

Lee [CONTRIBUTING.md](CONTRIBUTING.md). En resumen: `main` siempre estable, ramas `feature/*` (o `fix/*`), *conventional commits*, PR con la CI en verde y *squash merge*.

## Licencia

[MIT](LICENSE) © Ramses Sotomayor
