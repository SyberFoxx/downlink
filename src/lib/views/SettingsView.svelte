<script lang="ts">
  import { onMount } from "svelte";
  import {
    appFolders,
    appVersion,
    checkFfmpegUpdate,
    checkJsRuntimeUpdate,
    checkYtdlpUpdate,
    defaultDownloadDir,
    detectBrowsers,
    engineInfo,
    openExternal,
    pickCookiesFile,
    pickDownloadFolder,
    probeUrl,
    trayAvailable,
    updateFfmpeg,
    updateJsRuntime,
    updateYtdlp,
  } from "$lib/api";
  import EngineUpdateRow from "$lib/components/EngineUpdateRow.svelte";
  import Logo from "$lib/components/Logo.svelte";
  import { settings } from "$lib/settings.svelte";
  import { toast } from "$lib/toast.svelte";
  import { extractError, isAndroid } from "$lib/utils";
  import type { BrowserInfo, CookiesMode, EngineInfo, ShareAction } from "$lib/types";

  /** Repositorio del proyecto, para enlazarlo desde «Acerca de». */
  const REPO_URL = "https://github.com/SyberFoxx/downlink";
  /** Documentación en línea (el README del repositorio). */
  const DOCS_URL = `${REPO_URL}#readme`;

  /**
   * Prueba que exige cookies: YouTube responde «Sign in to confirm you're not a
   * bot» a las IP que considera sospechosas, así que este botón solo sirve para
   * comprobar que las cookies están bien puestas.
   */
  const TEST_URL_YOUTUBE = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
  /**
   * Prueba que **no** necesita cookies: un vídeo libre de Wikimedia Commons.
   * Sirve para saber si el motor funciona cuando el problema es solo YouTube;
   * si esta falla, el fallo está en la conexión o en los motores, no en las
   * cookies.
   */
  const TEST_URL_NO_COOKIES =
    "https://commons.wikimedia.org/wiki/File:Big_Buck_Bunny_medium.ogv";

  let browsers = $state<BrowserInfo[]>([]);
  let engine = $state<EngineInfo | null>(null);
  let loaded = $state(false);
  let saving = $state(false);
  let saved = $state(false);

  /** Carpeta de descargas del sistema (se muestra como referencia). */
  let systemDir = $state<string | null>(null);
  /** Carpetas propias de la app (Android), donde sí se puede escribir. */
  let androidFolders = $state<string[]>([]);
  /** Versión del binario, para la sección «Acerca de». */
  let version = $state("");

  /**
   * Nombre corto y legible de una carpeta propia de la app. La ruta completa es
   * larguísima (`/storage/emulated/0/Android/data/com.ramses.downlink/files/…`),
   * así que se muestra solo la parte que el usuario reconoce.
   */
  function shortFolder(folder: string): string {
    const last = folder.split("/").filter(Boolean).pop() ?? folder;
    return last === "files" ? "General" : last;
  }
  /** `true`/`false` si el sistema confirma; `null` si aún no se sabe. */
  let trayOk = $state<boolean | null>(null);

  /**
   * Qué prueba está en marcha, o `null`. Se usa para que cada botón muestre
   * «Probando…» solo el suyo.
   */
  let testing = $state<"youtube" | "no-cookies" | null>(null);
  let testResult = $state<{ ok: boolean; message: string } | null>(null);

  const modes: { id: CookiesMode; label: string; description: string }[] = [
    {
      id: "none",
      label: "Sin cookies",
      description: "Para sitios públicos que no exigen sesión",
    },
    {
      id: "browser",
      label: "Desde el navegador",
      description: "Recomendado para YouTube: usa tu sesión iniciada",
    },
    {
      id: "file",
      label: "Archivo cookies.txt",
      description: isAndroid()
        ? "Recomendado: expórtalo en el ordenador y pásalo al teléfono"
        : "Si usas un navegador portátil o quieres control manual",
    },
  ];

  /// Modos que tienen sentido donde corre la app. En Android «desde el navegador»
  /// es imposible y además engañoso, así que no se muestra:
  ///
  /// - Cada app vive en su propia jaula (`/data/data/<paquete>`) y las cookies de
  ///   Chrome, Brave o Samsung son **suyas**, no nuestras: no se pueden leer. Es
  ///   el mismo aislamiento que impide ejecutar binarios fuera de
  ///   `nativeLibraryDir`.
  /// - Aunque se pudieran copiar, Android las cifra con una clave del sistema:
  ///   harían falta root o ser el propio navegador.
  /// - `--cookies-from-browser` busca ficheros de perfil locales que en Android no
  ///   existen.
  ///
  /// La única vía real es pasar un `cookies.txt`, así que ahí esa pasa a ser la
  /// opción recomendada.
  const availableModes = $derived(
    isAndroid() ? modes.filter((mode) => mode.id !== "browser") : modes,
  );

  const shareModes: { id: ShareAction; label: string; description: string }[] = [
    {
      id: "download",
      label: "Descargar directamente",
      description:
        "Empieza a descargar con las opciones de la vista Descargar, sin pasos intermedios",
    },
    {
      id: "preview",
      label: "Mostrar la vista previa",
      description:
        "Te deja elegir calidad, formato y carpeta antes de descargar",
    },
  ];

  onMount(async () => {
    // Carga tolerante: si algo falla, los botones se habilitan igualmente y el
    // usuario ve el aviso. Lo importante es que `loaded` se marque **pase lo que
    // pase**: si `settings.load()` revienta (en Android puede, porque toca
    // rutas y servicios que no existen), antes se salía de la función sin pasar
    // por el `finally` y los botones —«Probar conexión», «Guardar»— se quedaban
    // deshabilitados para siempre.
    try {
      await settings.load();
      const [detected, info, defaultDir, hasTray, ownFolders, ver] =
        await Promise.all([
          detectBrowsers(),
          engineInfo(),
          defaultDownloadDir(),
          trayAvailable(),
          isAndroid() ? appFolders() : Promise.resolve([]),
          appVersion().catch(() => ""),
        ]);
      browsers = detected;
      engine = info;
      systemDir = defaultDir;
      trayOk = hasTray;
      androidFolders = ownFolders;
      version = ver;
      if (!settings.value.cookiesBrowser && detected.length > 0) {
        settings.value.cookiesBrowser = detected[0].id;
      }
    } catch (e) {
      toast.error(
        `No se pudo leer el estado de este equipo. ${extractError(e)}`,
      );
    } finally {
      loaded = true;
    }
  });

  async function refreshEngine() {
    try {
      engine = await engineInfo();
    } catch {
      // Si falla, se conserva la información previa.
    }
  }

  async function save() {
    saving = true;
    saved = false;
    // `settings.save()` ya avisa con toast si falla; solo celebramos si fue bien.
    const ok = await settings.save();
    if (ok) {
      saved = true;
      setTimeout(() => (saved = false), 2500);
    }
    saving = false;
  }

  async function chooseFile() {
    try {
      const file = await pickCookiesFile();
      if (file) settings.value.cookiesFile = file;
    } catch (e) {
      toast.error(`No se pudo abrir el selector de archivos. ${extractError(e)}`);
    }
  }

  async function chooseFolder() {
    try {
      const folder = await pickDownloadFolder();
      if (folder) settings.value.downloadDir = folder;
    } catch (e) {
      toast.error(`No se pudo abrir el selector de carpetas. ${extractError(e)}`);
    }
  }

  async function testConnection(url: string) {
    testing = url === TEST_URL_YOUTUBE ? "youtube" : "no-cookies";
    testResult = null;
    try {
      await settings.save();
      const info = await probeUrl(url);
      testResult = {
        ok: true,
        message: `Funciona: "${info.title}" (${info.formatsCount} formatos disponibles)`,
      };
    } catch (e) {
      testResult = {
        ok: false,
        message: typeof e === "string" ? e : "La prueba falló",
      };
    } finally {
      testing = null;
    }
  }

  let browserMissing = $derived(
    settings.value.cookiesMode === "browser" && browsers.length === 0,
  );
  let fileMissing = $derived(
    settings.value.cookiesMode === "file" && !settings.value.cookiesFile,
  );
</script>

<div class="mx-auto max-w-2xl px-10 py-12">
  <header class="mb-8">
    <h1 class="text-lg font-semibold tracking-tight text-ink">Ajustes</h1>
    <p class="mt-1 text-sm text-ink-muted">
      Configuración del motor de descargas
    </p>
  </header>

  <!-- Cookies -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Cookies</h2>
    <p class="mb-4 text-xs text-ink-muted">
      {#if isAndroid()}
        YouTube exige verificación anti-bot cada vez más seguido. En Android la única
        forma de usar tu sesión es un archivo <span class="text-ink">cookies.txt</span>
        exportado desde el navegador de tu ordenador.
      {:else}
        YouTube exige verificación anti-bot cada vez más seguido. Con las cookies de tu
        navegador (sesión iniciada) las descargas funcionan de forma confiable.
      {/if}
    </p>

    <div class="space-y-2">
      {#each availableModes as mode (mode.id)}
        <label
          class="flex cursor-pointer items-start gap-3 rounded-lg border px-3.5 py-2.5 transition-colors {settings.value.cookiesMode ===
          mode.id
            ? 'border-accent/60 bg-accent/5'
            : 'border-line bg-surface/30 hover:border-line-strong'}"
        >
          <input
            type="radio"
            name="cookies-mode"
            value={mode.id}
            bind:group={settings.value.cookiesMode}
            class="mt-0.5 size-3.5 max-md:size-5 accent-accent"
          />
          <span class="min-w-0">
            <span class="block text-sm text-ink">{mode.label}</span>
            <span class="block text-xs text-ink-muted">{mode.description}</span>
          </span>
        </label>
      {/each}
    </div>

    {#if settings.value.cookiesMode === "browser"}
      <div class="mt-3">
        {#if browserMissing}
          <p class="text-xs text-warn">
            No se detectó ningún navegador con perfil en este equipo.
          </p>
        {:else}
          <span class="mb-1 block text-xs text-ink-muted">Navegador</span>
          <select
            bind:value={settings.value.cookiesBrowser}
            aria-label="Navegador del que extraer cookies"
            class="tap w-full rounded-lg border border-line bg-surface/50 px-3 py-2 text-sm text-ink outline-none focus:border-accent"
          >
            {#each browsers as browser (browser.id)}
              <option value={browser.id}>{browser.label}</option>
            {/each}
          </select>
          <p class="mt-1.5 text-[11px] text-ink-faint">
            Debe estar iniciada la sesión en YouTube. Si falla, cierra el navegador e
            intenta de nuevo.
          </p>
        {/if}
      </div>
    {:else if settings.value.cookiesMode === "file"}
      <div class="mt-3">
        <div class="flex gap-2">
          <input
            type="text"
            bind:value={settings.value.cookiesFile}
            placeholder="/ruta/a/cookies.txt"
            class="tap min-w-0 flex-1 rounded-lg border border-line bg-surface/50 px-3 py-2 text-sm text-ink outline-none placeholder:text-ink-faint focus:border-accent"
          />
          <button
            class="tap rounded-lg border border-line px-3 text-sm text-ink-strong transition-colors hover:bg-surface"
            onclick={chooseFile}
          >
            Elegir…
          </button>
        </div>
        {#if fileMissing}
          <p class="mt-1.5 text-[11px] text-warn">
            Selecciona un archivo cookies.txt
          </p>
        {/if}
      </div>
    {/if}
  </section>

  <!-- Motor -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Motor</h2>
    <div
      class="rounded-lg border border-line bg-surface/30 px-3.5 py-3 text-sm"
    >
      <div class="flex items-center justify-between py-0.5">
        <span class="text-ink-muted">yt-dlp</span>
        <span class="flex items-center gap-2">
          <span class="font-mono text-xs text-ink-strong">
            {engine?.ytdlpVersion ?? "no encontrado"}
          </span>
          {#if engine?.ytdlpSource}
            <span
              class="rounded bg-elevated px-1.5 py-0.5 text-[10px] text-ink-soft"
              title={engine.ytdlpPath ?? undefined}
            >
              {engine.ytdlpSource}
            </span>
          {/if}
        </span>
      </div>
      <div class="flex items-center justify-between py-0.5">
        <span class="text-ink-muted">Runtime JS (desafíos YouTube)</span>
        {#if engine?.jsRuntime}
          <span class="flex items-center gap-2">
            <span class="font-mono text-xs text-ok">
              {engine.jsRuntime} ✓
            </span>
            {#if engine.jsRuntimeSource}
              <span class="rounded bg-elevated px-1.5 py-0.5 text-[10px] text-ink-soft">
                {engine.jsRuntimeSource}
              </span>
            {/if}
          </span>
        {:else}
          <span class="text-xs text-warn">
            no detectado — necesario para YouTube
          </span>
        {/if}
      </div>
      <div class="flex items-center justify-between py-0.5">
        <span class="text-ink-muted">ffmpeg (conversión y audio)</span>
        {#if engine?.ffmpegVersion}
          <span class="flex items-center gap-2">
            <span class="font-mono text-xs text-ok">
              {engine.ffmpegVersion} ✓
            </span>
            {#if engine.ffmpegSource}
              <span
                class="rounded bg-elevated px-1.5 py-0.5 text-[10px] text-ink-soft"
                title={engine.ffmpegPath ?? undefined}
              >
                {engine.ffmpegSource}
              </span>
            {/if}
          </span>
        {:else}
          <span class="text-xs text-warn">
            no detectado — instálalo para convertir formatos
          </span>
        {/if}
      </div>
    </div>

    <!-- Actualización de los motores -->
    <EngineUpdateRow
      engine="yt-dlp"
      name="yt-dlp"
      idleLabel="Buscar actualizaciones de yt-dlp"
      autoLabel="Buscar actualizaciones al iniciar la app"
      bind:autoChecked={settings.value.autoUpdateYtdlp}
      check={checkYtdlpUpdate}
      update={updateYtdlp}
      onChanged={refreshEngine}
    />

    <EngineUpdateRow
      engine="ffmpeg"
      name="ffmpeg"
      idleLabel="Descargar ffmpeg"
      missingLabel="No hay un ffmpeg que funcione en el sistema"
      autoLabel="Descargar ffmpeg al iniciar si falta"
      hint={isAndroid()
        ? "ffmpeg va incluido con la aplicación: no hay que descargarlo ni actualizarlo."
        : "Los builds de ffmpeg pesan ~150 MB. Si ya tienes ffmpeg instalado se respeta el tuyo."}
      bundled={isAndroid()}
      bind:autoChecked={settings.value.autoDownloadFfmpeg}
      check={checkFfmpegUpdate}
      update={updateFfmpeg}
      onChanged={refreshEngine}
    />

    <EngineUpdateRow
      engine="quickjs"
      name="runtime JS"
      idleLabel="Descargar runtime JS (quickjs)"
      missingLabel="YouTube necesita un runtime JS y no hay ninguno disponible"
      autoLabel="Descargar el runtime JS al iniciar si falta"
      hint={isAndroid()
        ? "quickjs va incluido con la aplicación: no hay que descargarlo ni actualizarlo."
        : "Es un binario diminuto (~2,5 MB) para resolver los desafíos de YouTube; evita tener que instalar Node.js. Si ya tienes node, deno o bun, se respeta el tuyo."}
      bundled={isAndroid()}
      bind:autoChecked={settings.value.autoDownloadJsRuntime}
      check={checkJsRuntimeUpdate}
      update={updateJsRuntime}
      onChanged={refreshEngine}
    />
  </section>

  <!-- Compartir desde otras apps (solo Android) -->
  {#if isAndroid()}
    <section class="mb-8">
      <h2 class="mb-1 text-sm font-medium text-ink-strong">
        Compartir desde otras apps
      </h2>
      <p class="mb-4 text-xs text-ink-muted">
        Al compartir un enlace desde YouTube o cualquier otra app, Downlink lo recibe
        al momento. Elige qué quieres que haga con él.
      </p>

      <div class="space-y-2">
        {#each shareModes as mode (mode.id)}
          <label
            class="flex cursor-pointer items-start gap-3 rounded-lg border px-3.5 py-2.5 transition-colors {settings.value
              .shareAction === mode.id
              ? 'border-accent/60 bg-accent/5'
              : 'border-line bg-surface/30 hover:border-line-strong'}"
          >
            <input
              type="radio"
              name="share-action"
              value={mode.id}
              bind:group={settings.value.shareAction}
              class="mt-0.5 size-3.5 max-md:size-5 accent-accent"
            />
            <span class="min-w-0">
              <span class="block text-sm text-ink">{mode.label}</span>
              <span class="block text-xs text-ink-muted">{mode.description}</span>
            </span>
          </label>
        {/each}
      </div>
    </section>
  {/if}

  <!-- Cola -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Cola de descargas</h2>
    <p class="mb-4 text-xs text-ink-muted">
      Más descargas simultáneas aprovechan mejor la conexión, pero compiten entre sí
      y con el resto de tus aplicaciones.
    </p>
    <div class="flex items-center gap-3">
      <select
        bind:value={settings.value.concurrency}
        aria-label="Descargas simultáneas"
        class="tap w-40 rounded-lg border border-line bg-surface/50 px-3 py-2 text-sm text-ink outline-none focus:border-accent"
      >
        {#each [1, 2, 3, 4, 6, 8] as n (n)}
          <option value={n}>{n} simultánea{n === 1 ? "" : "s"}</option>
        {/each}
      </select>
    </div>
    <p class="mt-1.5 text-[11px] text-ink-faint">
      Aplica de inmediato: si subes el número, arrancan los pendientes de la cola.
    </p>
  </section>

  <!-- Descargas y comportamiento -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Descargas</h2>
    <p class="mb-4 text-xs text-ink-muted">
      Dónde se guardan los archivos y qué ocurre al terminar o al cerrar la ventana.
    </p>

    <span class="mb-1 block text-xs text-ink-muted">Carpeta de destino</span>
    {#if isAndroid()}
      <!-- En Android no se ofrece el selector del sistema: devuelve un
           `content://` que el motor de descarga no puede usar como carpeta. Se
           eligen las carpetas propias de la app, que sí son rutas reales y no
           piden permisos. -->
      <select
        value={settings.value.downloadDir ?? ""}
        aria-label="Carpeta de destino de las descargas"
        class="tap w-full rounded-lg border border-line bg-surface/50 px-3 py-2 text-sm text-ink outline-none focus:border-accent"
        onchange={(event) => {
          const value = event.currentTarget.value;
          settings.value.downloadDir = value || null;
        }}
      >
        <option value="">Predeterminada</option>
        {#each androidFolders as folder (folder)}
          <option value={folder}>{shortFolder(folder)}</option>
        {/each}
      </select>
      <p class="mt-1.5 text-[11px] text-ink-faint">
        El vídeo o audio terminado se copia a la galería del teléfono, así que lo
        verás en tu app de Fotos o de Música. Esta carpeta solo decide dónde se
        prepara la descarga mientras llega.
      </p>
    {:else}
      <div class="flex flex-wrap gap-2">
        <input
          type="text"
          readonly
          value={settings.value.downloadDir ?? ""}
          placeholder={systemDir
            ? `Sistema · ${systemDir}`
            : "Carpeta de Descargas del sistema"}
          aria-label="Carpeta de destino de las descargas"
          class="tap min-w-0 flex-1 rounded-lg border border-line bg-surface/50 px-3 py-2 text-sm text-ink outline-none placeholder:text-ink-faint focus:border-accent"
        />
        <button
          class="tap rounded-lg border border-line px-3 text-sm text-ink-strong transition-colors hover:bg-surface"
          onclick={chooseFolder}
        >
          Elegir…
        </button>
        {#if settings.value.downloadDir}
          <button
            class="tap rounded-lg border border-line px-3 text-sm text-ink-soft transition-colors hover:bg-surface hover:text-ink"
            onclick={() => (settings.value.downloadDir = null)}
          >
            Usar la del sistema
          </button>
        {/if}
      </div>
      <p class="mt-1.5 text-[11px] text-ink-faint">
        Se crea si no existe. Se aplica a las descargas nuevas.
      </p>
    {/if}

    <div class="mt-4 space-y-3">
      <label class="tap flex cursor-pointer items-start gap-3">
        <input
          type="checkbox"
          bind:checked={settings.value.persistQueue}
          class="mt-0.5 size-3.5 max-md:size-5 accent-accent"
        />
        <span class="min-w-0">
          <span class="block text-sm text-ink">Reanudar la cola al abrir la app</span>
          <span class="block text-xs text-ink-muted">
            Las descargas pendientes se recuperan y continúan donde quedaron.
          </span>
        </span>
      </label>

      <label class="tap flex cursor-pointer items-start gap-3">
        <input
          type="checkbox"
          bind:checked={settings.value.notifyOnFinish}
          class="mt-0.5 size-3.5 max-md:size-5 accent-accent"
        />
        <span class="min-w-0">
          <span class="block text-sm text-ink">Avisar al terminar cada descarga</span>
          <span class="block text-xs text-ink-muted">
            Notificación del sistema cuando la ventana no está delante.
          </span>
        </span>
      </label>

      <label
        class="tap flex items-start gap-3 {trayOk === false
          ? 'cursor-not-allowed opacity-50'
          : 'cursor-pointer'}"
      >
        <input
          type="checkbox"
          bind:checked={settings.value.closeToTray}
          disabled={trayOk === false}
          class="mt-0.5 size-3.5 max-md:size-5 accent-accent"
        />
        <span class="min-w-0">
          <span class="block text-sm text-ink">
            Mantener en la bandeja al cerrar la ventana
          </span>
          <span class="block text-xs text-ink-muted">
            {trayOk === false
              ? "La bandeja del sistema no está disponible en este equipo."
              : "Las descargas siguen en marcha; vuelve desde el icono de la bandeja."}
          </span>
        </span>
      </label>
    </div>
  </section>

  <!-- Prueba -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Probar configuración</h2>
    <p class="mb-3 text-xs text-ink-muted">
      Analiza un video de prueba para comprobar que el motor responde. La prueba
      de YouTube necesita cookies; la otra no, así que sirve para distinguir un
      problema de cookies de uno de conexión.
    </p>
    <div class="flex flex-wrap items-center gap-3">
      <button
        class="tap rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-accent-ink shadow-sm shadow-accent/30 transition-colors hover:bg-accent-hover disabled:opacity-50"
        disabled={testing !== null || !loaded}
        onclick={() => testConnection(TEST_URL_YOUTUBE)}
      >
        {testing === "youtube" ? "Probando…" : "Probar con YouTube"}
      </button>
      <button
        class="tap rounded-md border border-line px-3.5 py-1.5 text-sm font-medium text-ink-strong transition-colors hover:bg-surface disabled:opacity-50"
        disabled={testing !== null || !loaded}
        onclick={() => testConnection(TEST_URL_NO_COOKIES)}
      >
        {testing === "no-cookies" ? "Probando…" : "Probar sin cookies"}
      </button>
      {#if testResult}
        <p
          class="copyable max-w-2xl whitespace-pre-wrap text-xs {testResult.ok
            ? 'text-ok'
            : 'text-danger'}"
          role="status"
        >
          {testResult.ok ? "✅" : "❌"}
          {testResult.message}
        </p>
      {/if}
    </div>
  </section>

  <!-- Acerca de -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Acerca de</h2>
    <p class="mb-4 text-xs text-ink-muted">
      Qué es Downlink, cómo empezar y dónde encontrar más información.
    </p>

    <div class="rounded-lg border border-line bg-surface/30 px-3.5 py-3.5">
      <div class="flex items-center gap-3">
        <Logo size={40} />
        <div class="min-w-0">
          <p class="text-sm font-medium text-ink-strong">
            Downlink{#if version} <span class="text-ink-faint">v{version}</span>{/if}
          </p>
          <p class="text-xs text-ink-muted">
            Descarga vídeo y audio de más de 1800 sitios.
          </p>
        </div>
      </div>

      <!-- Guía rápida -->
      <ol class="mt-4 space-y-2.5 text-xs text-ink-muted">
        <li class="flex gap-2.5">
          <span
            class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[10px] font-semibold text-accent-soft"
            >1</span
          >
          <span>
            Pega un enlace en <span class="text-ink-strong">Descargas</span> y
            pulsa el botón. Si activas la vista previa, antes podrás elegir
            calidad, formato y carpeta.
          </span>
        </li>
        <li class="flex gap-2.5">
          <span
            class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[10px] font-semibold text-accent-soft"
            >2</span
          >
          <span>
            {#if isAndroid()}
              Al terminar, el archivo se copia a la <span class="text-ink-strong"
                >galería del teléfono</span
              >: lo verás en tu app de Fotos o Música.
            {:else}
              Al terminar, el archivo queda en la
              <span class="text-ink-strong">carpeta de destino</span> que hayas
              elegido arriba.
            {/if}
          </span>
        </li>
        <li class="flex gap-2.5">
          <span
            class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[10px] font-semibold text-accent-soft"
            >3</span
          >
          <span>
            Si un sitio (YouTube, Instagram…) pide sesión, importa tu archivo
            <span class="text-ink-strong">cookies.txt</span> en la sección Cookies.
            Sin él, esos sitios pueden responder que no te reconoce.
          </span>
        </li>
        <li class="flex gap-2.5">
          <span
            class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full bg-accent/15 text-[10px] font-semibold text-accent-soft"
            >4</span
          >
          <span>
            ¿Algo no funciona? Usa <span class="text-ink-strong"
              >Probar configuración</span
            >
            más arriba: el botón «sin cookies» distingue un fallo de conexión de
            uno de cookies.
          </span>
        </li>
      </ol>

      <!-- Enlaces -->
      <div class="mt-4 flex flex-wrap gap-2 border-t border-line/70 pt-3.5">
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-xs text-ink-strong transition-colors hover:bg-surface"
          onclick={() => openExternal(REPO_URL)}
        >
          Repositorio en GitHub
        </button>
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-xs text-ink-strong transition-colors hover:bg-surface"
          onclick={() => openExternal(DOCS_URL)}
        >
          Documentación
        </button>
      </div>
      <p class="mt-2.5 text-[11px] text-ink-faint">
        Open source (MIT) · potenciado por yt-dlp y ffmpeg
      </p>
    </div>
  </section>

  <!-- Guardar -->
  <div class="flex items-center gap-3">
    <button
      class="tap rounded-md border border-line-strong px-4 py-1.5 text-sm text-ink transition-colors hover:bg-surface disabled:opacity-50"
      disabled={saving || !loaded}
      onclick={save}
    >
      {saving ? "Guardando…" : "Guardar cambios"}
    </button>
    {#if saved}
      <span class="text-xs text-ok">Guardado ✓</span>
    {/if}
  </div>
</div>
