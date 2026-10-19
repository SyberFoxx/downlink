<script lang="ts">
  import { onMount } from "svelte";
  import { pickWallpaper } from "$lib/api";
  import { initTheme, refreshPalette, theme } from "$lib/theme.svelte";
  import { settings } from "$lib/settings.svelte";
  import { autoThemeId, themeById, themeFromPalette, type Theme } from "$lib/themes";
  import { isAndroid } from "$lib/utils";

  onMount(() => {
    void initTheme();
  });

  let themeError = $state<string | null>(null);
  let adopting = $state(false);

  const AUTO_THEME: Theme = {
    id: "system",
    label: "Automático",
    appearance: "dark",
    tokens: themeById("downlink-dark")!.tokens,
  };

  /** Vista previa del tema "automático" (la que se aplicaría ahora). */
  let autoPreview = $derived(
    theme.palette
      ? themeFromPalette(theme.palette)
      : themeById(autoThemeId(theme.systemDark))!,
  );

  let activeId = $derived(settings.value.theme.themeId);
  let currentAccent = $derived(
    settings.value.theme.accent ?? theme.active.tokens.accent,
  );
  let opacityPct = $derived(
    Math.round(settings.value.theme.wallpaperOpacity * 100),
  );
  let blurPx = $derived(Math.round(settings.value.theme.wallpaperBlur));

  const ACCENTS = [
    "#7c3aed",
    "#6366f1",
    "#3b82f6",
    "#06b6d4",
    "#10b981",
    "#84cc16",
    "#f59e0b",
    "#f97316",
    "#ef4444",
    "#ec4899",
    "#a855f7",
    "#64748b",
  ];

  function preview(themeId: string): Theme {
    return themeId === "system" ? autoPreview : (themeById(themeId) ?? AUTO_THEME);
  }

  async function chooseWallpaper() {
    themeError = null;
    let path: string | null = null;
    try {
      path = await pickWallpaper();
    } catch (e) {
      // En Android el selector lo abre el lado nativo; si falla ahí (por ejemplo,
      // sin ninguna aplicación de galería), el error llega hasta aquí.
      themeError =
        typeof e === "string" ? e : "No se pudo abrir el selector de imágenes.";
      return;
    }
    if (!path) return;
    try {
      await theme.setWallpaper(path);
    } catch (e) {
      themeError = typeof e === "string" ? e : "No se pudo cargar la imagen.";
    }
  }

  async function adoptPalette() {
    adopting = true;
    themeError = null;
    try {
      const ok = await theme.adoptPalette();
      if (!ok) {
        themeError =
          "No se encontró ninguna paleta de pywal o wallust en ~/.cache.";
      }
    } finally {
      adopting = false;
    }
  }
</script>

<div class="mx-auto max-w-2xl px-10 py-12">
  <header class="mb-8">
    <h1 class="text-lg font-semibold tracking-tight text-ink">Apariencia</h1>
    <p class="mt-1 text-sm text-ink-muted">
      Dale tu toque: tema, color de acento e imagen de fondo.
    </p>
  </header>

  <!-- Tema -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Tema</h2>
    <p class="mb-4 text-xs text-ink-muted">
      «Automático» sigue el modo claro/oscuro del sistema y, si encuentra una
      paleta de pywal o wallust, imita los colores de tu escritorio.
    </p>

    <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
      <button
        class="rounded-lg border p-2 text-left transition-colors {activeId ===
        'system'
          ? 'border-accent/60 bg-accent/5'
          : 'border-line hover:border-line-strong'}"
        onclick={() => theme.setTheme("system")}
      >
        <div
          class="relative h-12 overflow-hidden rounded-md"
          style="background:{autoPreview.tokens.canvas}"
        >
          <div class="absolute inset-2 flex flex-col justify-between">
            <span
              class="h-2 w-8 rounded-full"
              style="background:{autoPreview.tokens.accent}"
            ></span>
            <span
              class="h-3 w-full rounded"
              style="background:{autoPreview.tokens.surface}"
            ></span>
          </div>
          <span
            class="absolute right-1.5 top-1.5 text-[10px] font-medium"
            style="color:{autoPreview.tokens.inkMuted}"
            >auto</span
          >
        </div>
        <div class="mt-2 flex items-center justify-between gap-2">
          <span class="truncate text-xs text-ink-strong">Automático</span>
          {#if activeId === "system"}
            <span class="text-xs text-accent">✓</span>
          {/if}
        </div>
      </button>

      {#each ["downlink-dark", "downlink-light"] as id (id)}
        {@const t = preview(id)}
        <button
          class="rounded-lg border p-2 text-left transition-colors {activeId ===
          id
            ? 'border-accent/60 bg-accent/5'
            : 'border-line hover:border-line-strong'}"
          onclick={() => theme.setTheme(id)}
        >
          <div
            class="h-12 overflow-hidden rounded-md"
            style="background:{t.tokens.canvas}"
          >
            <div class="flex h-full flex-col justify-between p-2">
              <span
                class="h-2 w-8 rounded-full"
                style="background:{t.tokens.accent}"
              ></span>
              <span
                class="h-3 w-full rounded"
                style="background:{t.tokens.surface}"
              ></span>
            </div>
          </div>
          <div class="mt-2 flex items-center justify-between gap-2">
            <span class="truncate text-xs text-ink-strong">{t.label}</span>
            {#if activeId === id}<span class="text-xs text-accent">✓</span>{/if}
          </div>
        </button>
      {/each}
    </div>

    <div class="mt-3 grid grid-cols-2 gap-3 sm:grid-cols-3">
      {#each [
        "catppuccin-mocha",
        "catppuccin-latte",
        "nord",
        "dracula",
        "gruvbox-dark",
        "tokyo-night",
        "rose-pine",
        "solarized-light",
      ] as id (id)}
        {@const t = preview(id)}
        <button
          class="rounded-lg border p-2 text-left transition-colors {activeId ===
          id
            ? 'border-accent/60 bg-accent/5'
            : 'border-line hover:border-line-strong'}"
          onclick={() => theme.setTheme(id)}
        >
          <div
            class="h-12 overflow-hidden rounded-md"
            style="background:{t.tokens.canvas}"
          >
            <div class="flex h-full flex-col justify-between p-2">
              <span
                class="h-2 w-8 rounded-full"
                style="background:{t.tokens.accent}"
              ></span>
              <span
                class="h-3 w-full rounded"
                style="background:{t.tokens.surface}"
              ></span>
            </div>
          </div>
          <div class="mt-2 flex items-center justify-between gap-2">
            <span class="truncate text-xs text-ink-strong">{t.label}</span>
            {#if activeId === id}<span class="text-xs text-accent">✓</span>{/if}
          </div>
        </button>
      {/each}
    </div>
  </section>

  <!-- Acento -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Color de acento</h2>
    <p class="mb-4 text-xs text-ink-muted">
      Se usa en botones, foco y barras de progreso. Déjalo en «Del tema» para
      usar el del tema elegido.
    </p>

    <div class="flex flex-wrap items-center gap-2">
      <button
        class="tap rounded-md border px-2.5 py-1 text-xs transition-colors {settings
          .value.theme.accent === null
          ? 'border-accent/60 text-ink-strong'
          : 'border-line text-ink-muted hover:border-line-strong'}"
        onclick={() => theme.setAccent(null)}
      >
        Del tema
      </button>
      {#each ACCENTS as color (color)}
        <button
          aria-label={`Acento ${color}`}
          class="tap tap-center size-6 rounded-full transition-transform hover:scale-110"
          onclick={() => theme.setAccent(color)}
        >
          <span
            class="size-6 rounded-full border {settings.value.theme.accent ===
            color
              ? 'border-ink ring-2 ring-accent/50'
              : 'border-line'}"
            style="background:{color}"
          ></span>
        </button>
      {/each}
      <label
        class="tap tap-center flex cursor-pointer"
        title="Color personalizado"
      >
        <span
          class="flex size-6 items-center justify-center overflow-hidden rounded-full border border-line"
        >
          <input
            type="color"
            class="size-9 cursor-pointer border-0 bg-transparent p-0"
            value={currentAccent}
            oninput={(e) =>
              (settings.value.theme.accent = e.currentTarget.value)}
            onchange={() => theme.setAccent(settings.value.theme.accent)}
          />
        </span>
      </label>
    </div>
  </section>

  <!-- Fondo -->
  <section class="mb-8">
    <h2 class="mb-1 text-sm font-medium text-ink-strong">Imagen de fondo</h2>
    <p class="mb-4 text-xs text-ink-muted">
      {#if isAndroid()}
        Una imagen de tu galería por detrás de los paneles. Se guarda una copia
        dentro de la app, así que puedes borrar la original sin perder el fondo.
      {:else}
        Una imagen tuya (wallpaper) por detrás de los paneles. Cuanto más
        translúcido el tema, más se nota.
      {/if}
    </p>

    {#if theme.wallpaperUrl}
      <div class="mb-3 overflow-hidden rounded-lg border border-line">
        <div
          class="h-32 w-full bg-cover bg-center"
          style="background-image:url('{theme.wallpaperUrl}')"
        ></div>
      </div>

      <div class="space-y-3">
        <label class="block">
          <span class="mb-1 flex items-center justify-between text-xs">
            <span class="text-ink-muted">Intensidad</span>
            <span class="text-ink-faint">{opacityPct}%</span>
          </span>
          <input
            type="range"
            min="0"
            max="100"
            value={opacityPct}
            class="tap w-full accent-accent"
            oninput={(e) => {
              const v = Number(e.currentTarget.value);
              settings.value.theme.wallpaperOpacity = v / 100;
            }}
            onchange={() =>
              theme.setWallpaperLook({
                opacity: settings.value.theme.wallpaperOpacity,
              })}
          />
        </label>

        <label class="block">
          <span class="mb-1 flex items-center justify-between text-xs">
            <span class="text-ink-muted">Desenfoque</span>
            <span class="text-ink-faint">{blurPx}px</span>
          </span>
          <input
            type="range"
            min="0"
            max="60"
            value={blurPx}
            class="tap w-full accent-accent"
            oninput={(e) => {
              const v = Number(e.currentTarget.value);
              settings.value.theme.wallpaperBlur = v;
            }}
            onchange={() =>
              theme.setWallpaperLook({
                blur: settings.value.theme.wallpaperBlur,
              })}
          />
        </label>
      </div>

      <div class="mt-4 flex items-center gap-2">
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-sm text-ink-strong transition-colors hover:bg-surface"
          onclick={chooseWallpaper}
        >
          Cambiar imagen
        </button>
        <button
          class="tap rounded-md border border-danger-bg/60 px-3 py-1.5 text-sm text-danger transition-colors hover:bg-danger-bg/60"
          onclick={() => theme.clearWallpaper()}
        >
          Quitar
        </button>
      </div>
    {:else}
      <button
        class="tap rounded-md border border-line px-3 py-1.5 text-sm text-ink-strong transition-colors hover:bg-surface"
        onclick={chooseWallpaper}
      >
        Elegir imagen…
      </button>
    {/if}
  </section>

  <!-- Escritorio -->
  {#if !isAndroid()}
    <section class="mb-8">
      <h2 class="mb-1 text-sm font-medium text-ink-strong">Escritorio</h2>
      <p class="mb-4 text-xs text-ink-muted">
        Si usas pywal o wallust (típico en Hyprland y otros escritorios), Downlink
        puede adoptar la paleta que generan a partir de tu fondo.
      </p>

      <div class="rounded-lg border border-line bg-surface/30 px-3.5 py-3 text-sm">
        {#if theme.palette}
          <div class="flex items-center justify-between">
            <span class="text-ink-muted">Paleta detectada</span>
            <span class="font-mono text-xs text-ok">{theme.palette.source}</span>
          </div>
          <div class="mt-2 flex gap-1">
            {#each theme.palette.colors.slice(0, 8) as color, i (i)}
              <span
                class="h-4 flex-1 rounded-sm"
                style="background:{color}"
              ></span>
            {/each}
          </div>
        {:else}
          <div class="flex items-center justify-between">
            <span class="text-ink-muted">Paleta detectada</span>
            <span class="text-xs text-ink-faint">ninguna</span>
          </div>
        {/if}
      </div>

      <div class="mt-3 flex items-center gap-2">
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-sm text-ink-strong transition-colors hover:bg-surface disabled:opacity-50"
          disabled={adopting || !theme.palette}
          onclick={adoptPalette}
        >
          Adoptar paleta
        </button>
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-sm text-ink-muted transition-colors hover:bg-surface"
          onclick={() => refreshPalette()}
        >
          Volver a buscar
        </button>
      </div>
    </section>
  {/if}

  <!-- Restablecer -->
  <div class="flex items-center gap-3">
    <button
      class="tap rounded-md border border-line px-4 py-1.5 text-sm text-ink-strong transition-colors hover:bg-surface"
      onclick={() => theme.reset()}
    >
      Restablecer apariencia
    </button>
  </div>

  {#if themeError}
    <p class="mt-3 text-xs text-warn">{themeError}</p>
  {/if}
</div>
