# Android (v1.0) — plan de port

Auditoría de preparación de Downlink para Android **arm64** y plan de trabajo por
fases. Las referencias son del árbol en v0.10.0 (`archivo:línea`).

- **Alcance**: Android arm64 (y, si se quiere, armv7/x86_64 después).
- **Fuera de alcance**: **iOS**. La arquitectura de Downlink se apoya en lanzar
  `yt-dlp` y `ffmpeg` como **procesos externos**, y iOS no permite crear
  procesos hijos desde una app. No es un problema de esfuerzo, es de plataforma.
- **Estado**: fase A (interfaz móvil) casi lista: el armazón ya está adaptado y
  medido en navegador. Quedan los **gestos** para reordenar la cola y el pegado
  por *share intent*; después, el andamiaje Android y el motor nativo.

## Resumen

| | |
|---|---|
| **Bloqueantes** | Ejecutar el motor (W^X), acceso al almacenamiento (SAF), scaffold Android inexistente |
| **A adaptar** | UI táctil, ventana, bandeja/notificaciones, cookies/navegadores, pywal/fondo, "mostrar en carpeta", CI |
| **Ya preparado** | Tokens de tema, cola/historial/persistencia, TLS rustls, plugins, iconos Android, frontend estático |

La buena noticia: la app está bien separada. **El sistema de temas, el estado y
la persistencia no se tocan**; lo que falta es la capa de interfaz y el
empaquetado del motor.

---

## 1. Bloqueantes

### 1.1. Ejecutar yt-dlp/ffmpeg (el bloqueante central)

Todo el motor se invoca como **proceso externo** y se resuelve por `PATH`,
sidecar o copia gestionada:

- `src-tauri/src/ytdlp.rs:456` y `:461` — `probe()` lanza `Command::new(ytdlp_bin())`.
- `src-tauri/src/ytdlp.rs:722` y `:730` — `spawn_process()` lanza el proceso con
  `current_dir(out_dir)`.
- `src-tauri/src/ytdlp.rs:787`, `:804` — ejecución de binarios para verificar versión.
- `src-tauri/src/queue.rs:381` — `pump()` llama a `ytdlp::spawn_process(...)`.
- `src-tauri/src/engine.rs:50` — `init()` fija `<app_data>/bin`.
- `src-tauri/src/engine.rs:71` — `managed_path()` (copia gestionada).
- `src-tauri/src/engine.rs:76` — `sidecar_path()` (junto al ejecutable).
- `src-tauri/src/engine.rs:83` — `which()` recorre el `PATH` del sistema.
- `src-tauri/src/engine.rs:93` — `resolve()`: la cadena de precedencia.

**Problema**: Android 10+ (API 29) aplica **W^X**: no se puede ejecutar un
binario dejado en el directorio de datos de la app. La copia gestionada en
`<app_data>/bin` no puede funcionar, y en Android no hay `PATH` de usuario.

**Solución**: empaquetar yt-dlp y ffmpeg **como bibliotecas nativas**
(`jniLibs/arm64-v8a/lib*.so`), que Android extrae a `nativeLibraryDir` y ese
directorio **sí** es ejecutable. En Android, `resolve()` debe ir directo a esa
ruta y saltarse `PATH`, sidecar y `<app_data>/bin`.

El actualizador de motores **no aplica** en Android:

- `src-tauri/src/updater.rs:408` — `update_ytdlp()` (descarga y `rename`).
- `src-tauri/src/updater.rs:727` — `update_ffmpeg()`.
- `src-tauri/src/updater.rs:864` — `managed_dir()` crea `<app_data>/bin`.
- `src-tauri/src/updater.rs:276` — `make_executable()` con `chmod 0755`.
- `src-tauri/src/updater.rs:880` — `auto_update()` al arrancar.
- `src-tauri/src/lib.rs:296` — el `setup()` lanza `updater::auto_update`.

Los assets que descarga hoy son builds de **GNU/Linux** (glibc), no de Android
(Bionic): no sirven tal cual.

### 1.2. Almacenamiento y rutas

- `src-tauri/src/settings.rs:215` — `download_dir()` (carpeta del sistema o la
  configurada, con `create_dir_all`).
- `src-tauri/src/settings.rs:204` — `configured_dir()` acepta cualquier `PathBuf`.
- `src-tauri/src/lib.rs:237` — `default_download_dir` expone la ruta del sistema.
- `src-tauri/src/queue.rs:381` — la carpeta se pasa como `current_dir`.

Android usa **almacenamiento por alcance**: no se puede escribir en rutas
arbitrarias ni usar `content://` como `current_dir`. Hace falta **SAF/MediaStore**
para el destino final y un directorio privado para el `.part` y el procesado.

### 1.3. Scaffold Android inexistente

- No existe `src-tauri/gen/android` (solo `src-tauri/gen/schemas/`).
- `src-tauri/capabilities/default.json:2` — la capability usa
  `desktop-schema.json`; hay que crear una de móvil (`mobile-schema`).
- `src-tauri/tauri.conf.json:30` — `bundle.targets: "all"`, sin sección Android.

### 1.4. Runtime JS (YouTube)

- `src-tauri/src/ytdlp.rs:27` — `detect_js_runtime()` busca deno/node/bun en el `PATH`.
- `src-tauri/src/ytdlp.rs:73` — `--js-runtimes` con lo que encuentre.
- `src-tauri/src/ytdlp.rs:88` — `--remote-components ejs:github` (solo YouTube).

En Android no habrá runtime JS en el sistema: hay que empaquetarlo o resolver los
desafíos de otra forma, igual que con el motor.

---

## 2. Requiere adaptación

### 2.1. Interfaz (lo más visible)

Medido sobre el código actual:

| Problema | Medida real | Efecto en móvil |
|---|---|---|
| Barra lateral fija `src/lib/components/Sidebar.svelte:45` (`w-56`) | 224 px fijos | En 360 px ocupa el **62%** de la pantalla |
| Media queries responsive | Solo `src/app.css:87` (`prefers-reduced-motion`) | No hay ni un breakpoint |
| Tipografía | 66× `text-sm`, 64× `text-xs`, 32× `text-[11px]`, 5× `text-[10px]` | Letra de escritorio en 6" |
| Objetivos táctiles | 14× `size-3.5` (14 px), 9× `size-4`, `p-1` (4 px) | Mínimo recomendado: 44×44 px |
| Alto de pantalla `src/routes/+page.svelte:138` (`h-screen`) | `100vh` | No encaja con las barras dinámicas de Android → hay que usar `100dvh` |
| Safe areas | 0 usos de `env(safe-area-inset-*)` | Contenido bajo el notch y la barra de gestos |
| `hover` | 61 usos de `hover:`, **1** de `active:` | Tailwind v4 los envuelve en `@media (hover: hover)`, así que **en táctil no se aplican**; falta feedback de pulsación (`active:`) |
| Drag & drop HTML5 | `src/lib/components/QueueList.svelte:39-72`, `src/lib/components/DownloadRow.svelte:57-65` | Reordenar con el dedo no funciona |
| Pegado | `Ctrl+V` en `src/routes/+page.svelte:95-107` | No existe: hace falta share intent |
| Fondo con desenfoque | `src/app.css:101-110` | Caro en GPU móvil (opcional) |

**El sistema de temas en sí no se rompe**: son tokens CSS
(`src/app.css:7-26`, `src/app.css:30-55`) aplicados desde
`src/lib/theme.svelte.ts:30-40`, y el modo claro/oscuro sigue a
`prefers-color-scheme` (`src/lib/theme.svelte.ts:64-68`), que Android soporta.

### 2.2. Ventana

- `src-tauri/tauri.conf.json:18-19` — `minWidth: 940`, `minHeight: 600`: sin
  sentido en móvil. Hay que hacer la configuración de ventana condicional.

### 2.3. Bandeja, cerrar a bandeja y notificaciones

Ya hay gating `cfg(desktop)`, pero hay que rematar el caso móvil:

- `src-tauri/src/lib.rs:7` — `#[cfg(desktop)] mod tray;`
- `src-tauri/src/lib.rs:275` — `tray::setup` (solo escritorio).
- `src-tauri/src/lib.rs:302` — cerrar a bandeja (solo escritorio).
- `src-tauri/src/queue.rs:589` / `:610` / `:637` — notificación y tooltip.
  En no-escritorio la notificación es **no-op**: en Android hay que activarla y
  pedir el permiso de notificaciones en tiempo de ejecución.

### 2.4. Cookies y navegadores

- `src-tauri/src/settings.rs:136` — `detect_browsers()`: lee perfiles de `HOME`,
  sin sentido en Android.
- `src-tauri/src/ytdlp.rs:60` — `--cookies-from-browser`: no aplica. En móvil hay
  que ofrecer solo **importar `cookies.txt`** (vía SAF) y adaptar los mensajes de
  error que hoy mandan a Ajustes → cookies del navegador.

### 2.5. Paleta del escritorio y fondo

- `src-tauri/src/theme.rs:145` — `read_system_palette()` (pywal/wallust) es de
  escritorio; en Android devolverá `None` y el tema cae al claro/oscuro por
  defecto (`src/lib/theme.svelte.ts:80-86`). Degradación limpia, pero la UI de
  Apariencia debe ocultar esa sección.
- `src-tauri/src/theme.rs:71` — `read_wallpaper()` lee una **ruta** de fichero.
  En Android la imagen tiene que venir de un selector del sistema.

### 2.6. "Mostrar en carpeta"

`revealItemInDir` no existe en Android (`src/lib/downloads.svelte.ts:212-215`,
`src/lib/views/HistoryView.svelte:50-53`): sustituir por **abrir/compartir**.

### 2.7. CI

- `.github/workflows/ci.yml` — sin target Android.
- `.github/workflows/release.yml` — sin job de APK/AAB ni Android SDK/NDK.

---

## 3. Ya está preparado

- **Entrada móvil declarada**: `src-tauri/src/lib.rs:253`
  (`#[cfg_attr(mobile, tauri::mobile_entry_point)]`).
- **Cola, historial y persistencia** (`queue.rs`, `history.rs`, `queue_store.rs`)
  son agnósticos de plataforma y usan `app_config_dir()`, que funciona en Android.
- **TLS con rustls** (`src-tauri/Cargo.toml:30`), sin OpenSSL del sistema.
- **Plugins** ya integrados (`src-tauri/Cargo.toml:24-26`, `src-tauri/src/lib.rs:259-261`):
  opener, dialog y notification (el más fácil de portar a móvil).
- **Iconos Android ya generados**: `src-tauri/icons/android/**`.
- **Frontend estático sin SSR** (adapter-static) y `viewport` correcto.
- **Tema por tokens**: cambiar de tema no recompila CSS.

---

## 4. Fases

### Fase A — Interfaz móvil (primero)

Objetivo: que la app **se vea bien** en una pantalla de móvil, verificable en el
navegador a 390 px sin necesidad de dispositivo ni toolchain.

El breakpoint elegido es el `md` de Tailwind (768 px), que cae justo por debajo
del ancho mínimo que la ventana de escritorio ya respeta (`tauri.conf.json:18`),
así que la disposición de escritorio no se toca. Las medidas de abajo salen de un
navegador real a 390×844.

- [x] Breakpoint de móvil (768 px) sin alterar la disposición de escritorio
      (escritorio: 224 px de barra / 1056 px de contenido, idéntico a antes).
- [x] Barra lateral → **navegación inferior** (390×65, en fila y pegada abajo).
- [x] `100dvh` en vez de `100vh` y `env(safe-area-inset-*)` arriba y abajo.
- [x] Tipografía base a 17 px en móvil (todo lo que mide en `rem` escala).
- [x] Desactivar en móvil el fondo con desenfoque.
- [x] Objetivos táctiles de la navegación a 51 px de alto (era 32 px).
- [x] Objetivos táctiles ≥ 44 px en el resto de botones e iconos. El más pequeño
      medía 26 px; ahora **ninguno baja de 44** en ninguna de las seis pantallas
      (cola, opciones, playlist, historial, apariencia y ajustes), medido en un
      navegador real con `hover: none` y `pointer: coarse`. Se resuelve con `.tap`
      (mínimo 44 px) y `.tap-center` (centra el icono) en `app.css`, solo en la
      franja de móvil, así que el escritorio conserva sus tamaños.
- [x] La barra de progreso pasa a lo ancho, debajo de la fila, en móvil: junto a
      los botones de 44 px no cabía y se quedaba en 0 px (ahora mide 263 px).
- [x] Los avisos emergentes se colocan por encima de la navegación inferior (antes
      la tapaban); su altura vive en `--nav-height`.
- [x] Feedback de pulsación en los controles: donde no hay ratón (`hover: none` y
      `pointer: coarse`) todo lo pulsable se atenúa al presionarlo. El escritorio
      sigue igual.
- [ ] Reordenar la cola con **gestos** (Pointer Events) en vez de HTML5 drag & drop.
- [x] `user-select: none` revisado: el texto que conviene copiar (el error de una
      descarga, la ruta del archivo, el resultado de la prueba de conexión) lleva
      `.copyable` y se puede seleccionar; el resto de la interfaz sigue sin
      seleccionarse a propósito (`app.css:64`).
- [ ] Pegado por *share intent* en vez de `Ctrl+V`.

### Fase B — Andamiaje Android

- [ ] `tauri android init` (crea `src-tauri/gen/android`).
- [ ] `minSdk`/`targetSdk` y permisos (INTERNET, FOREGROUND_SERVICE, POST_NOTIFICATIONS).
- [ ] Capability de móvil (`mobile-schema`).
- [ ] Configuración de ventana condicional (`tauri.conf.json:18-19`).
- [ ] Job de CI para APK/AAB.

### Fase C — Motores nativos (arm64)

- [ ] yt-dlp y ffmpeg como `.so` arm64 (Bionic, no glibc) en `jniLibs`.
- [ ] `resolve()` con ruta fija a `nativeLibraryDir` en Android (`engine.rs:93`).
- [ ] Desactivar el actualizador en Android (`updater.rs`, `lib.rs:296`) y ocultar
      sus comandos/UI.
- [ ] Runtime JS para YouTube (empaquetado o alternativa).
- [ ] Verificar el `spawn` real en dispositivo (`ytdlp.rs:730`, `queue.rs:381`).

### Fase D — Almacenamiento y salida

- [ ] Directorio privado para `.part`/procesado y destino final por SAF/MediaStore.
- [ ] `default_download_dir` y selector de carpeta adaptados.
- [ ] Sustituir "mostrar en carpeta" por abrir/compartir.
- [ ] Cookies solo por fichero; ocultar detección de navegadores.

### Fase E — Notificaciones y ciclo de vida

- [ ] Activar notificaciones en móvil y pedir el permiso.
- [ ] Gestionar segundo plano (foreground service si se quiere descarga en background).
- [ ] Revisar la cola persistente ante el matado de proceso por el sistema.

### Fase F — Firma y publicación

- [ ] Firma, `versionCode`/`versionName`, minSdk definitivo.
- [ ] APK/AAB en `release.yml`.

---

## 5. Riesgos a vigilar

1. **`color-mix` y WebView antiguo**: hay **82** usos de opacidad tipo
   `bg-surface/30`, que Tailwind compila a `color-mix(in oklab, …)` (necesita
   WebView **111+**, 2023). Mitigado: Tailwind emite además un respaldo dentro de
   `@supports (color: color-mix(in lab, red, red))`, así que en un WebView sin
   soporte el color se aplica **a plena opacidad** en vez de desaparecer. Se
   degrada el matiz, no se rompe la interfaz. Aun así conviene fijar una versión
   mínima de WebView.
2. **libc**: los binarios de Android usan Bionic, no glibc. Conseguir/compilar
   yt-dlp y ffmpeg para arm64 Android es la parte con más incógnitas.
3. **Peso del APK**: yt-dlp + ffmpeg + runtime JS por arquitectura.
4. **W^X**: confirmar en dispositivo real desde qué directorio se puede ejecutar.

## 6. Cómo probar sin dispositivo

La ventana de Tauri no sirve para esto porque tiene `minWidth: 940`
(`src-tauri/tauri.conf.json:18`). Hay dos caminos:

1. **A ojo**, en el navegador: `npm run dev` y DevTools → modo dispositivo →
   **390×844**.
2. **Con medidas** (lo que se usó para verificar la fase A): la app se pinta en el
   cliente y sin las APIs de Tauri **no se dibuja nada** (no es un problema de
   CSS: revienta en `onMount` al no existir `window.__TAURI_INTERNALS__`), así que
   hace falta un *stub* de Tauri y un reportero que mida la geometría real
   (`getBoundingClientRect`, tamaños de los objetivos táctiles, desbordamiento
   horizontal) y la envíe a un servidor local. Con eso se comprueba el layout con
   números en vez de "a ojo", que es como se midieron las cifras de la fase A.

Datos clave a 390×844 después de la fase A: `main` a 390 px de ancho (antes 166),
barra inferior de 390×65 con botones de 91×51 (antes 207×32 dentro de una barra
lateral de 224 px), tipografía raíz 17 px y sin desbordamiento horizontal.
