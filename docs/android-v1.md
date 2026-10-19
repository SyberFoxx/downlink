# Android (v1.0) — plan de port

Auditoría de preparación de Downlink para Android **arm64** y plan de trabajo por
fases. Las referencias son del árbol en v0.10.0 (`archivo:línea`).

- **Alcance**: Android arm64 (y, si se quiere, armv7/x86_64 después).
- **Fuera de alcance**: **iOS**. La arquitectura de Downlink se apoya en lanzar
  `yt-dlp` y `ffmpeg` como **procesos externos**, y iOS no permite crear
  procesos hijos desde una app. No es un problema de esfuerzo, es de plataforma.
- **Estado**: fases A y B encaminadas. El armazón móvil está adaptado y medido en
  navegador; los enlaces compartidos desde otras apps se reciben **de punta a punta**
  (menú «Compartir» → `MainActivity` → JNI → `SharedLink` → interfaz) y la app ya
  **construye un APK instalable** (ver 7). Quedan los **gestos** para reordenar la
  cola, el destino de descargas por SAF y, sobre todo, los **motores nativos**. La
  duda de fondo —cómo ejecutar el motor— ya está decidida: los intérpretes van dentro
  del APK y yt-dlp se pone al día solo (5.4).

## Resumen

| | |
|---|---|
| **Bloqueantes** | Ejecutar el motor (W^X: **resuelto** en 5.4; falta empaquetarlo), acceso al almacenamiento (SAF) |
| **A adaptar** | UI táctil, ventana, bandeja/notificaciones, cookies/navegadores, pywal/fondo, "mostrar en carpeta", CI |
| **Ya preparado** | Tokens de tema, cola/historial/persistencia, TLS rustls, plugins, iconos Android, frontend estático, andamiaje Android y *share intent* |

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

**Solución**: los **intérpretes y herramientas** (Python, ffmpeg, quickjs) van
empaquetados como bibliotecas nativas (`jniLibs/arm64-v8a/lib*.so`), que Android
extrae a `nativeLibraryDir`, el único directorio ejecutable para una app normal. En
Android, `resolve()` debe ir directo a esa ruta y saltarse `PATH`, sidecar y
`<app_data>/bin`.

**Ojo con `extractNativeLibs`** (esto costó un APK aparentemente correcto que no
funcionaba): a partir de `minSdk 23`, AGP pone `android:extractNativeLibs="false"`
por defecto y los `lib*.so` **quedan mapeados dentro del APK**, sin escribirse en
disco. Para *cargar* una biblioteca da igual, pero aquí los motores son
**ejecutables**: sin extracción no hay nada que lanzar y `nativeLibraryDir` no
existe como carpeta real. Hay que forzar la extracción en los dos sitios
(`app/build.gradle.kts` con `packaging { jniLibs { useLegacyPackaging = true } }`
y `AndroidManifest.xml` con `android:extractNativeLibs="true"`). Se comprueba con
`aapt2 dump xmltree --file AndroidManifest.xml <apk>`.

La ruta de `nativeLibraryDir` la calcula Android en cada instalación, así que no se
adivina: `MainActivity.onCreate` se la manda a Rust por JNI
(`Java_com_ramses_downlink_MainActivity_pushNativeLibraryDir` →
`engine::set_bundled_lib_dir`). `engine::detect_bundled_lib_dir()` (vía `dladdr`)
queda solo como respaldo.

`yt-dlp` **no** es un binario y no va por ahí: es un *zipapp* de Python que se
**lee**, no se ejecuta, así que puede seguir viviendo en el directorio de datos y
actualizarse solo. El detalle y por qué es compatible con Play, en 5.4.

El actualizador de motores **sí aplica** en Android, pero solo para `yt-dlp`:

- `src-tauri/src/updater.rs:408` — `update_ytdlp()` (descarga y `rename`): sirve
  casi tal cual; cambia el asset y que no hay que dar permiso de ejecución.
- `src-tauri/src/updater.rs:727` — `update_ffmpeg()`: no aplica (va en el APK).
- `src-tauri/src/updater.rs:864` — `managed_dir()` crea `<app_data>/bin`; en
  Android el *zipapp* vive aquí, pero no se ejecuta desde aquí.
- `src-tauri/src/updater.rs:276` — `make_executable()` (`chmod 0755`): no hace
  falta, el *zipapp* se lee.
- `src-tauri/src/updater.rs:880` — `auto_update()` al arrancar: sigue valiendo.
- `src-tauri/src/lib.rs:296` — el `setup()` lanza `updater::auto_update`.

Los assets que descarga hoy son builds de **GNU/Linux** (glibc) y binarios; en
Android el asset es el *zipapp* `yt-dlp` (código interpretado) y el resto de los
motores van dentro del APK.

### 1.2. Almacenamiento y rutas

- `src-tauri/src/settings.rs:215` — `download_dir()` (carpeta del sistema o la
  configurada, con `create_dir_all`).
- `src-tauri/src/settings.rs:204` — `configured_dir()` acepta cualquier `PathBuf`.
- `src-tauri/src/lib.rs:237` — `default_download_dir` expone la ruta del sistema.
- `src-tauri/src/queue.rs:381` — la carpeta se pasa como `current_dir`.

Android usa **almacenamiento por alcance**: no se puede escribir en rutas
arbitrarias ni usar `content://` como `current_dir`. Por eso el `.part` y el
procesado ocurren en el directorio privado de la app y el destino final se
resuelve con **MediaStore** (ver Fase D): se descarga en
`Android/data/<paquete>/files/` y, al terminar, el archivo se publica en la
biblioteca multimedia con `MediaStoreBridge.publish`, que devuelve el
`content://` de la entrada visible en la galería.

### 1.3. Scaffold Android (ya existe)

- `src-tauri/gen/android` está generado (`tauri android init`) y el primer APK se
  construye y firma (ver 7). Lo que falta de aquí:
- `src-tauri/capabilities/default.json:2` — la capability usa
  `desktop-schema.json`; hay que crear una de móvil (`mobile-schema`).
- `src-tauri/tauri.conf.json:30` — `bundle.targets: "all"`, sin sección Android.

### 1.4. Runtime JS (YouTube)

- `src-tauri/src/ytdlp.rs:27` — `detect_js_runtime()` busca deno/node/bun en el `PATH`.
- `src-tauri/src/ytdlp.rs:73` — `--js-runtimes` con lo que encuentre.
- `src-tauri/src/ytdlp.rs:88` — `--remote-components ejs:github` (solo YouTube).

En Android no hay runtime JS en el sistema: se empaqueta `quickjs` como
`lib*.so` en el APK (se ejecuta desde `nativeLibraryDir`, igual que ffmpeg). A
diferencia de yt-dlp, un runtime JS es un binario, así que no se actualiza solo:
su versión es la de la app.

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
- En móvil las cookies se van a necesitar más que en escritorio: muchos sitios
  públicos (YouTube entre ellos) funcionan sin ellas, pero Instagram, Facebook,
  X/Twitter y todo lo privado o con restricción de edad piden sesión cada vez más
  a menudo. El mensaje de error tiene que decir qué hacer **en Android**
  (`cookies.txt`), no ofrecer un navegador que no existe.
- El selector de archivos del sistema devuelve un `content://` que yt-dlp no sabe
  abrir como fichero: hay que copiarlo al directorio privado de la app y pasar esa
  ruta.

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

### 2.7. Enlaces compartidos (*share intent*)

Lo que ya está hecho y probado **sin dispositivo** (navegador simulando el WebView
de Android, ver 6):

- `extractUrls()` se queda con la URL limpia: fuera la puntuación que arrastra el
  texto que la envuelve («… /vídeo.» o «(…?list=PL123)»), pero dentro los
  paréntesis que sí son parte de la URL (una desambiguación de Wikipedia).
- `analyze()` acepta el bloque entero que comparte otra app (título + enlace).
  Antes se le pasaba tal cual a yt-dlp, que respondía
  `'Un vídeo…\nhttps://…' is not a valid URL`.
- Si el texto no trae ningún enlace, se avisa («No he encontrado ningún enlace en
  el texto recibido») en vez de no hacer nada, que dejaba la app con pinta de rota.
- Ajuste **Compartir desde otras apps** (solo se muestra en Android, por el
  agente de usuario): descargar directamente con las opciones de la vista
  Descargar —el flujo de pegar el enlace en la consola— o dejar la vista previa.
  Por defecto, descargar. Una lista de reproducción nunca se descarga sola: cae a
  la vista previa para que se elija.
- La interfaz escucha `share-intent` y, al arrancar, recoge el enlace que quedó
  pendiente (`take_shared_text`). Las entregas repetidas del mismo texto en menos
  de dos segundos se descartan.

El puente nativo ya está hecho (fase B):

- `AndroidManifest.xml`: filtro `SEND` + `text/plain`, así que Downlink aparece en
  el menú «Compartir» de cualquier app (YouTube, Instagram, Chrome, TikTok…), que
  era el objetivo. `launchMode="singleTask"` ya lo traía la plantilla.
- `MainActivity.kt` lee el enlace en `onCreate` (el caso normal al compartir es el
  arranque en frío) **y** en `onNewIntent` (si Downlink ya estaba abierto), y
  acepta `EXTRA_TEXT` o, si no viene ahí, `ClipData`, que es por donde lo mandan
  unas pocas apps.
- `src-tauri/src/share.rs` recibe ese texto por JNI
  (`Java_com_ramses_downlink_MainActivity_pushSharedText`), lo guarda en
  `SharedLink` y emite `share-intent`. Es la única vía que no pasa por la webview,
  que en frío todavía no existe.
- Varios enlaces compartidos de una vez van todos a la cola, igual que al pegarlos.

Falta probarlo en un dispositivo. Y hasta la fase C (motores) compartir un enlace
llega, pero no puede descargar: el aviso lo dice tal cual —«Esta versión para Android
todavía no incluye el motor de descarga»— en vez de un error del sistema que
parecería una avería.

### 2.8. CI

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
- [x] `user-select: none` revisado: el texto que conviene copiar (el error de una
      descarga, la ruta del archivo, el resultado de la prueba de conexión) lleva
      `.copyable` y se puede seleccionar; el resto de la interfaz sigue sin
      seleccionarse a propósito (`app.css:64`).
- [x] Enlaces **compartidos desde otras apps**: al llegar el texto completo de
      otra app (título + enlace) se extrae la URL, con la puntuación que arrastra
      el texto alrededor fuera y los paréntesis que sí son del enlace dentro; si
      no hay ningún enlace, se avisa en vez de no hacer nada. Un ajuste elige
      entre descargar ya (como pegar el enlace en la consola) o dejar la vista
      previa. Verificado en navegador simulando el WebView de Android, incluido el
      arranque desde cero (el enlace espera al otro lado y la app lo recoge).
- [ ] Reordenar la cola con **gestos** (Pointer Events) en vez de HTML5 drag & drop.
- [x] Puente nativo del *share intent* (`intent-filter` + JNI). Ver 2.7.

### Fase B — Andamiaje Android

- [x] `tauri android init` (crea `src-tauri/gen/android`) y primer APK.
- [x] `intent-filter` de `ACTION_SEND` (`text/plain`) con `launchMode="singleTask"`
      y puente JNI hasta `SharedLink` (ver 2.7). El enlace viaja en `EXTRA_TEXT`
      —`EXTRA_HTML_TEXT` no hace falta— y `MainActivity` lo lee en `onCreate` y en
      `onNewIntent`.
- [ ] Permisos que faltan para lo que viene: `FOREGROUND_SERVICE` y
      `POST_NOTIFICATIONS` (fase E). `INTERNET` ya lo declara la plantilla.
- [ ] Capability de móvil (`mobile-schema`).
- [ ] Configuración de ventana condicional (`tauri.conf.json:18-19`).
- [ ] Job de CI para APK/AAB.

### Fase C — Motores nativos (arm64)

- [x] CPython para Android (el ejecutable y su `libpython3.x.so`) compilado con el
      NDK y empaquetado en `jniLibs` con `android:extractNativeLibs="true"` (5.4).
- [x] ffmpeg y quickjs como `.so` arm64 (Bionic, no glibc) en `jniLibs`.
- [x] `ffprobe` al lado de ffmpeg (`libffprobe.so`): yt-dlp lo usa para leer
      duración, códecs y streams al incrustar metadatos o portada. Sin él el
      postprocesado falla con «Unable to embed using ffprobe & ffmpeg» (5.4).
- [x] Certificados raíz de TLS (`cacert.pem`) dentro del stdlib y `SSL_CERT_FILE`
      apuntándolos: sin ellos Python rechaza cualquier HTTPS (5.4).
- [x] `resolve()` con ruta fija a `nativeLibraryDir` en Android (`engine.rs:93`); la
      ruta la da el lado Kotlin (`applicationInfo.nativeLibraryDir`), porque cambia
      con cada instalación.
- [x] La invocación deja de ser un binario: `<python> <zipapp-yt-dlp>` más los
      mismos argumentos (`ytdlp.rs:456`, `:730`, `:787`).
- [x] Caso `android` en el actualizador: descarga del *zipapp* `yt-dlp` con
      reemplazo atómico y sin `chmod` (`updater.rs:341`, `:408`).
- [x] Ajustes en Android: las filas de «comprobar/actualizar» y los interruptores de
      descarga automática cuentan que ffmpeg y el runtime JS vienen con la app, y que
      solo yt-dlp se pone al día solo.
- [ ] App Bundle/`abiFilters`: no servir las tres arquitecturas al mismo teléfono.
- [ ] CI: `cargo check --target aarch64-linux-android` y la construcción de los
      motores, o el código `#[cfg(target_os = "android")]` no se compila nunca.
- [ ] Verificar el `spawn` real en dispositivo (`ytdlp.rs:730`, `queue.rs:381`).

### Fase D — Almacenamiento y salida

- [x] Directorio privado para `.part`/procesado y destino final por MediaStore.
      Android solo deja escribir sin permisos en `Android/data/<paquete>/files/`,
      que el usuario **no ve**, así que ahí se descarga y al terminar el archivo se
      publica en la biblioteca multimedia (`MediaStoreBridge.publish`), que crea la
      entrada visible en la galería y borra la copia privada. El `content://`
      resultante pasa a ser la ruta del trabajo.
- [x] `default_download_dir` y selector de carpeta adaptados: en Android no se
      ofrece el selector del sistema (devuelve un `content://` inutilizable como
      `current_dir`) sino las carpetas propias de la app (`app_folders`).
- [x] Sustituir "mostrar en carpeta" por abrir. `revealItemInDir` no existe en
      Android; el botón pasa a **abrir** el archivo (`open_file`), que acepta tanto
      rutas reales como `content://`.
- [x] Cookies solo por fichero; ocultar detección de navegadores. El selector de
      Android devuelve un `content://`, que yt-dlp no acepta como `--cookies`, así
      que el `cookies.txt` elegido se copia a la carpeta privada y se le pasa la
      ruta real (`pick_cookies_file`). Falta verificarlo en dispositivo.

**Tres detalles que costaron un intento fallido cada uno:**

- **R8 renombra los métodos que Rust llama por JNI.** `env.call_method(activity,
  "publishToMediaStore", …)` busca el nombre literal; con la minificación de
  release los métodos pasaban a llamarse `a`, `b`… y la llamada fallaba con
  `NoSuchMethodError`, silenciado por el propio código. Se conservan con
  `-keepclassmembers` en `proguard-rules.pro`.
- **La firma JNI tiene que coincidir exactamente.** `openDownloaded` devolvía
  `boolean` pero Rust usaba un puente común que espera `String`; ahora ambos
  métodos devuelven `String`, y el descriptor es el mismo para los dos.
- **Abrir no puede hacerse desde el hilo principal.** El comando `open_file` es
  `async` y el puente corre en `spawn_blocking`: hablar con el hilo principal
  desde el propio hilo principal se quedaría esperando su respuesta.

**Limitación conocida:** los subtítulos sueltos (`--write-subs` sin incrustar) no se
publican en la galería (solo el archivo principal tiene entrada). Sus ficheros, y
los restos de yt-dlp (`.part`, `.webp` de portada), se borran de la carpeta privada
al terminar la descarga (`queue.rs::cleanup_companions`), así que no se acumulan.

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
2. **libc**: los binarios de Android usan Bionic, no glibc. Hay que compilar
   Python, ffmpeg y quickjs para arm64 Android; yt-dlp no se compila (es Python y
   se ejecuta con el intérprete empaquetado, ver 5.4).
3. **Peso del APK**: Python + ffmpeg + quickjs + el *zipapp* de yt-dlp, por
   arquitectura.
4. **W^X — decidido: el motor va dentro del APK y yt-dlp se actualiza solo.**

   Desde Android 10, una app con `targetSdk ≥ 29` **no puede ejecutar binarios
   escritos en su directorio de datos** (es la razón por la que Termux sigue en
   `targetSdk 28` y no está en Play). La única carpeta ejecutable para una app
   normal es `nativeLibraryDir`: el instalador extrae ahí lo que el APK lleva en
   `lib/<abi>/`, así que un binario renombrado a `lib*.so` —con
   `android:extractNativeLibs="true"`, que Play permite— se puede lanzar sin root y
   con el `targetSdk` al día. La ruta cambia con cada instalación, así que hay que
   preguntársela a Android (`applicationInfo.nativeLibraryDir`) y pasársela a Rust:
   Tauri no la expone.

   **Lo que no puede actualizarse solo**: `ffmpeg` y `quickjs` son binarios. Van
   firmados dentro del APK y su versión es la de la app; actualizarlos es una
   versión nueva de Downlink. En Ajustes, en Android, sus filas de
   «comprobar/actualizar» y los interruptores de descarga automática deben contar
   eso, no ofrecer una descarga que no va a pasar.

   **Tres detalles que hay que acertar al empaquetar** (los tres costaron un APK
   que «no funcionaba»):

   - `android:extractNativeLibs="true"`: sin él los `.so` no se escriben en disco
     y no hay nada que ejecutar (ver 1.1).
   - `cacert.pem` dentro del stdlib: sin certificados raíz, Python arranca pero
     rechaza todo HTTPS con «unable to get local issuer certificate» y yt-dlp ni
     llega a YouTube. Se apunta con `SSL_CERT_FILE`/`REQUESTS_CA_BUNDLE`.
   - `ffprobe` junto a ffmpeg, como `libffprobe.so`: yt-dlp lo busca sustituyendo
     `ffmpeg` por `ffprobe` en el nombre que le pasa `--ffmpeg-location`, así que
     basta con dejar los dos ficheros al lado con esos nombres. Sin `ffprobe`, el
     postprocesado falla al incrustar metadatos o portada.

   **Lo que sí se actualiza solo: `yt-dlp`, porque es Python.** Y no hace falta
   bajar el `targetSdk` ni salirse de Play: la política de *Device and Network
   Abuse* de Google Play prohíbe descargar **código ejecutable** (`dex`, `JAR`,
   `.so`) de fuera de Play, pero **excluye expresamente el código interpretado**
   —«esta restricción no se aplica al código que corre en una máquina virtual o un
   intérprete»— y la propia política nombra Python. Así que:

   - Se empaqueta un **CPython para Android** (el ejecutable como `libpythonbin.so`
     y su `libpython3.x.so` al lado, con `LD_LIBRARY_PATH` apuntando a
     `nativeLibraryDir`). Desde 3.13 Android es plataforma soportada por CPython
     (PEP 738), así que se puede compilar con el NDK en CI.
   - `yt-dlp` es el ***zipapp* oficial de sus *releases*** (el asset `yt-dlp`, que
     empieza por `#!/usr/bin/env python3`): ~3 MB que **no se ejecutan, se leen**.
     Vive en el directorio de datos de la app y se actualiza con un reemplazo
     atómico (`rename`), así que no deja restos ni versiones viejas.
   - Se lleva además una copia del *zipapp* dentro del APK como punto de partida:
     la app funciona recién instalada y sin red, y el actualizador solo mejora lo
     que ya hay.
   - Con eso el comportamiento es el de escritorio: la app se pone al día sola y
     sigue funcionando cuando YouTube cambia algo.

   Comprobado con las herramientas reales: `python3 yt-dlp.zipapp --version` →
   `2026.08.19`, y el mismo `--dump-single-json` de la app sobre un enlace de
   prueba devuelve los metadatos igual que el binario (y `--ffmpeg-location`, que
   `common_args` ya añade cuando ffmpeg no está en el `PATH`, apunta al del APK).

   Coste a cambio: cada análisis y cada descarga arrancan un intérprete de más
   (unos milisegundos y unos MB por proceso). Es el precio de tener un motor que no
   caduca; un yt-dlp empaquetado en el APK arrancaría más rápido y dejaría de
   funcionar en cuanto YouTube cambie.

   **Por qué no la vía Termux** (`targetSdk 28`): Termux tampoco ejecuta un yt-dlp
   autocontenido —su `yt-dlp` es código Python que corre con el Python de Termux—,
   y no existe un yt-dlp de binario único para Bionic (los `yt-dlp_linux_aarch64`
   oficiales son glibc). Incluso bajando el `targetSdk` habría que llevar un
   intérprete, y a cambio se perdería Play y se dependería de ejecutar desde un
   directorio escribible. Se descarta: cuesta más y da menos.

   Lo que esto le pide al código (fase C):

   - Motores conscientes de Android: `python`, `ffmpeg` y `quickjs` desde
     `nativeLibraryDir`; yt-dlp desde el directorio de datos (o el del APK).
   - La invocación deja de ser «un binario»: pasa a ser un prefijo
     (`<python> <zipapp>`) más los mismos argumentos de siempre. `common_args` ya
     añade `--ffmpeg-location` cuando ffmpeg no viene del `PATH`, así que apuntar al
     del APK es casi gratis.
   - El actualizador gana un caso `android` que **sí** tiene sentido: no descargar
     un binario, sino el *zipapp* de yt-dlp.
   - `#[cfg(target_os = "android")]` **no se compila en Linux**: todo esto hay que
     comprobarlo en CI con el objetivo `aarch64-linux-android` activado
     (`cargo check --target aarch64-linux-android`), o el compilador no lo verá
     hasta que exista un dispositivo delante.

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

El mismo banco sirve para el **share intent** sin móvil: se le cambia el agente de
usuario al del WebView de Android (la app lo mira para saber dónde corre), el
*stub* responde a `take_shared_text` con el texto compartido y puede emitir
`share-intent` como haría Rust. Así se comprueban las dos vías —app cerrada (el
enlace espera) y app abierta (llega el evento)—, el ajuste de Ajustes y el
resultado en pantalla, con el texto real que manda cada app.

## 7. Compilar el APK

No hace falta Android Studio: con el SDK, el NDK y un JDK 17 —los tres ya
instalados en esta máquina— basta, más el *std* de Rust para Android
(`rustup target add aarch64-linux-android`). Los detalles que costaron un rato:

- `JAVA_HOME` tiene que apuntar a un JDK 17 **completo**:
  `/usr/lib/jvm/java-21-openjdk` existe pero está vacío, sin `bin/java`.
- `compileSdk`/`targetSdk` son 37; si la plataforma no está, Gradle la descarga.
- El NDK 28.2 ya alinea a páginas de 16 KB (requisito de Play), sin flags extra.

```sh
export ANDROID_HOME="$HOME/Android/sdk" ANDROID_SDK_ROOT="$HOME/Android/sdk" \
       NDK_HOME="$HOME/Android/sdk/ndk/28.2.13676358" \
       JAVA_HOME=/usr/lib/jvm/java-17-temurin-jdk
npx tauri android build --debug --apk --target aarch64
```

El APK queda en
`src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`
y va firmado con la clave de depuración, así que se instala tal cual. **Ojo con el
tamaño**: en depuración la librería lleva los símbolos y el APK ronda los 400 MB;
con `CARGO_PROFILE_DEV_DEBUG=0` en el entorno baja a lo razonable. Una compilación
de publicación lo deja en su sitio de verdad, pero exige configurar la firma (fase
F).

Para que el APK **ejecute** los motores hace falta que el instalador los extraiga
(ver 5.4): `AndroidManifest.xml` lleva `android:extractNativeLibs="true"` y
`build.gradle.kts` `packaging { jniLibs { useLegacyPackaging = true } }`. Si algún
día los motores «no aparecen» tras instalar, lo primero que hay que mirar es esto:

```sh
aapt2 dump xmltree --file AndroidManifest.xml <apk> | grep extractNativeLibs
# tiene que decir =true
unzip -l <apk> | grep lib/          # y aquí tienen que salir los lib*.so
```

Para comprobar el código `#[cfg(target_os = "android")]`, que en Linux no se compila
—y por tanto no lo ve ni `cargo check` ni Clippy—, sirve el mismo target con el
compilador del NDK en el `PATH`:

```sh
cargo check --target aarch64-linux-android
```
