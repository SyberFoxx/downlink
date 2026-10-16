<script lang="ts">
  import { onMount } from "svelte";
  import {
    checkFfmpegUpdate,
    checkYtdlpUpdate,
    detectBrowsers,
    engineInfo,
    getSettings,
    pickCookiesFile,
    probeUrl,
    saveSettings,
    updateFfmpeg,
    updateYtdlp,
  } from "$lib/api";
  import EngineUpdateRow from "$lib/components/EngineUpdateRow.svelte";
  import type {
    BrowserInfo,
    CookiesMode,
    EngineInfo,
    Settings,
  } from "$lib/types";

  const TEST_URL = "https://www.youtube.com/watch?v=jNQXAC9IVRw";

  let settings = $state<Settings>({
    cookiesMode: "none",
    cookiesBrowser: null,
    cookiesFile: null,
    concurrency: 2,
    autoUpdateYtdlp: true,
    autoDownloadFfmpeg: true,
  });
  let browsers = $state<BrowserInfo[]>([]);
  let engine = $state<EngineInfo | null>(null);
  let loaded = $state(false);
  let saving = $state(false);
  let saved = $state(false);

  let testing = $state(false);
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
      description: "Si usas un navegador portátil o quieres control manual",
    },
  ];

  onMount(async () => {
    const [loadedSettings, detected, info] = await Promise.all([
      getSettings(),
      detectBrowsers(),
      engineInfo(),
    ]);
    settings = loadedSettings;
    browsers = detected;
    engine = info;
    if (!settings.cookiesBrowser && detected.length > 0) {
      settings.cookiesBrowser = detected[0].id;
    }
    loaded = true;
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
    try {
      await saveSettings(settings);
      saved = true;
      setTimeout(() => (saved = false), 2500);
    } finally {
      saving = false;
    }
  }

  async function chooseFile() {
    const file = await pickCookiesFile();
    if (file) settings.cookiesFile = file;
  }

  async function testConnection() {
    testing = true;
    testResult = null;
    try {
      await saveSettings(settings);
      const info = await probeUrl(TEST_URL);
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
      testing = false;
    }
  }

  let browserMissing = $derived(
    settings.cookiesMode === "browser" && browsers.length === 0,
  );
  let fileMissing = $derived(
    settings.cookiesMode === "file" && !settings.cookiesFile,
  );
</script>

<div class="mx-auto max-w-2xl px-10 py-12">
  <header class="mb-8">
    <h1 class="text-lg font-semibold tracking-tight text-gray-100">Ajustes</h1>
    <p class="mt-1 text-sm text-gray-500">
      Configuración del motor de descargas
    </p>
  </header>

  <!-- Cookies -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-gray-300">Cookies</h2>
    <p class="mb-4 text-xs text-gray-500">
      YouTube exige verificación anti-bot cada vez más seguido. Con las cookies de tu
      navegador (sesión iniciada) las descargas funcionan de forma confiable.
    </p>

    <div class="space-y-2">
      {#each modes as mode (mode.id)}
        <label
          class="flex cursor-pointer items-start gap-3 rounded-lg border px-3.5 py-2.5 transition-colors {settings.cookiesMode ===
          mode.id
            ? 'border-violet-600/60 bg-violet-600/5'
            : 'border-gray-800 bg-gray-900/30 hover:border-gray-700'}"
        >
          <input
            type="radio"
            name="cookies-mode"
            value={mode.id}
            bind:group={settings.cookiesMode}
            class="mt-0.5 size-3.5 accent-violet-600"
          />
          <span class="min-w-0">
            <span class="block text-sm text-gray-200">{mode.label}</span>
            <span class="block text-xs text-gray-500">{mode.description}</span>
          </span>
        </label>
      {/each}
    </div>

    {#if settings.cookiesMode === "browser"}
      <div class="mt-3">
        {#if browserMissing}
          <p class="text-xs text-amber-400">
            No se detectó ningún navegador con perfil en este equipo.
          </p>
        {:else}
          <span class="mb-1 block text-xs text-gray-500">Navegador</span>
          <select
            bind:value={settings.cookiesBrowser}
            aria-label="Navegador del que extraer cookies"
            class="w-full rounded-lg border border-gray-800 bg-gray-900/50 px-3 py-2 text-sm text-gray-100 outline-none focus:border-violet-600"
          >
            {#each browsers as browser (browser.id)}
              <option value={browser.id}>{browser.label}</option>
            {/each}
          </select>
          <p class="mt-1.5 text-[11px] text-gray-600">
            Debe estar iniciada la sesión en YouTube. Si falla, cierra el navegador e
            intenta de nuevo.
          </p>
        {/if}
      </div>
    {:else if settings.cookiesMode === "file"}
      <div class="mt-3">
        <div class="flex gap-2">
          <input
            type="text"
            bind:value={settings.cookiesFile}
            placeholder="/ruta/a/cookies.txt"
            class="min-w-0 flex-1 rounded-lg border border-gray-800 bg-gray-900/50 px-3 py-2 text-sm text-gray-100 outline-none placeholder:text-gray-600 focus:border-violet-600"
          />
          <button
            class="rounded-lg border border-gray-800 px-3 text-sm text-gray-300 transition-colors hover:bg-gray-900"
            onclick={chooseFile}
          >
            Elegir…
          </button>
        </div>
        {#if fileMissing}
          <p class="mt-1.5 text-[11px] text-amber-400">
            Selecciona un archivo cookies.txt
          </p>
        {/if}
      </div>
    {/if}
  </section>

  <!-- Motor -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-gray-300">Motor</h2>
    <div
      class="rounded-lg border border-gray-800 bg-gray-900/30 px-3.5 py-3 text-sm"
    >
      <div class="flex items-center justify-between py-0.5">
        <span class="text-gray-500">yt-dlp</span>
        <span class="flex items-center gap-2">
          <span class="font-mono text-xs text-gray-300">
            {engine?.ytdlpVersion ?? "no encontrado"}
          </span>
          {#if engine?.ytdlpSource}
            <span
              class="rounded bg-gray-800 px-1.5 py-0.5 text-[10px] text-gray-400"
              title={engine.ytdlpPath ?? undefined}
            >
              {engine.ytdlpSource}
            </span>
          {/if}
        </span>
      </div>
      <div class="flex items-center justify-between py-0.5">
        <span class="text-gray-500">Runtime JS (desafíos YouTube)</span>
        {#if engine?.jsRuntime}
          <span class="font-mono text-xs text-emerald-400">
            {engine.jsRuntime} ✓
          </span>
        {:else}
          <span class="text-xs text-amber-400">
            no detectado — instala node o deno
          </span>
        {/if}
      </div>
      <div class="flex items-center justify-between py-0.5">
        <span class="text-gray-500">ffmpeg (conversión y audio)</span>
        {#if engine?.ffmpegVersion}
          <span class="flex items-center gap-2">
            <span class="font-mono text-xs text-emerald-400">
              {engine.ffmpegVersion} ✓
            </span>
            {#if engine.ffmpegSource}
              <span
                class="rounded bg-gray-800 px-1.5 py-0.5 text-[10px] text-gray-400"
                title={engine.ffmpegPath ?? undefined}
              >
                {engine.ffmpegSource}
              </span>
            {/if}
          </span>
        {:else}
          <span class="text-xs text-amber-400">
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
      bind:autoChecked={settings.autoUpdateYtdlp}
      check={checkYtdlpUpdate}
      update={updateYtdlp}
      onChanged={refreshEngine}
    />

    <EngineUpdateRow
      engine="ffmpeg"
      name="ffmpeg"
      idleLabel="Descargar ffmpeg"
      missingLabel="No se encontró ffmpeg en el sistema"
      autoLabel="Descargar ffmpeg al iniciar si falta"
      hint="Los builds de ffmpeg pesan ~150 MB. Si ya tienes ffmpeg instalado se respeta el tuyo."
      bind:autoChecked={settings.autoDownloadFfmpeg}
      check={checkFfmpegUpdate}
      update={updateFfmpeg}
      onChanged={refreshEngine}
    />
  </section>

  <!-- Cola -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-gray-300">Cola de descargas</h2>
    <p class="mb-4 text-xs text-gray-500">
      Más descargas simultáneas aprovechan mejor la conexión, pero compiten entre sí
      y con el resto de tus aplicaciones.
    </p>
    <div class="flex items-center gap-3">
      <select
        bind:value={settings.concurrency}
        aria-label="Descargas simultáneas"
        class="w-40 rounded-lg border border-gray-800 bg-gray-900/50 px-3 py-2 text-sm text-gray-100 outline-none focus:border-violet-600"
      >
        {#each [1, 2, 3, 4, 6, 8] as n (n)}
          <option value={n}>{n} simultánea{n === 1 ? "" : "s"}</option>
        {/each}
      </select>
    </div>
    <p class="mt-1.5 text-[11px] text-gray-600">
      Aplica de inmediato: si subes el número, arrancan los pendientes de la cola.
    </p>
  </section>

  <!-- Prueba -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-gray-300">Probar configuración</h2>
    <p class="mb-3 text-xs text-gray-500">
      Analiza un video de prueba en YouTube usando la configuración actual.
    </p>
    <div class="flex items-center gap-3">
      <button
        class="rounded-md bg-violet-600 px-3.5 py-1.5 text-sm font-medium text-white shadow-sm shadow-violet-600/30 transition-colors hover:bg-violet-500 disabled:opacity-50"
        disabled={testing || !loaded}
        onclick={testConnection}
      >
        {testing ? "Probando…" : "Probar conexión"}
      </button>
      {#if testResult}
        <p
          class="text-xs {testResult.ok ? 'text-emerald-400' : 'text-red-400'}"
        >
          {testResult.ok ? "✅" : "❌"}
          {testResult.message}
        </p>
      {/if}
    </div>
  </section>

  <!-- Guardar -->
  <div class="flex items-center gap-3">
    <button
      class="rounded-md border border-gray-700 px-4 py-1.5 text-sm text-gray-200 transition-colors hover:bg-gray-900 disabled:opacity-50"
      disabled={saving || !loaded}
      onclick={save}
    >
      {saving ? "Guardando…" : "Guardar cambios"}
    </button>
    {#if saved}
      <span class="text-xs text-emerald-400">Guardado ✓</span>
    {/if}
  </div>
</div>
