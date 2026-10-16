<script lang="ts">
  import { untrack } from "svelte";
  import type { DownloadOptions, ProbeInfo, SponsorBlockMode } from "$lib/types";
  import {
    AUDIO_FORMATS,
    AUDIO_QUALITIES,
    SPONSORBLOCK_CATEGORIES,
    VIDEO_CONTAINERS,
    PRESETS,
    availableHeightsFor,
    closestHeight,
    formatTimecode,
    parseTimecode,
    qualityApplies,
    summarizeOptions,
    type PresetId,
  } from "$lib/options";
  import { formatDuration } from "$lib/utils";

  interface Props {
    info: ProbeInfo;
    options: DownloadOptions;
    onDownload: () => void;
    starting: boolean;
    /** Texto del botón principal (p. ej. «Descargar 12 vídeos»). */
    label?: string;
    /** Deshabilita el botón principal (p. ej. ninguna selección). */
    disabled?: boolean;
  }

  let {
    info,
    options,
    onDownload,
    starting,
    label = "Descargar",
    disabled = false,
  }: Props = $props();

  let presetIds = $derived([
    "video-1080",
    "best-quality",
    "mp3-320",
    "flac",
    "audio-original",
  ] as PresetId[]);

  let activePreset = $state<PresetId | null>(null);

  const sponsorblockModes: { id: SponsorBlockMode; label: string }[] = [
    { id: "remove", label: "Quitar" },
    { id: "mark", label: "Marcar" },
  ];

  /** Marca o desmarca una categoría de SponsorBlock (crea un array nuevo). */
  function toggleCategory(id: string) {
    const selected = options.sponsorblockCategories;
    options.sponsorblockCategories = selected.includes(id)
      ? selected.filter((category) => category !== id)
      : [...selected, id];
  }

  // Recorte: texto editable en la UI, sincronizado a segundos en las opciones.
  // `untrack` marca la lectura inicial de las opciones como intencionada.
  let clipStartText = $state(untrack(() => formatTimecode(options.clipStart)));
  let clipEndText = $state(untrack(() => formatTimecode(options.clipEnd)));

  let clipError = $derived.by(() => {
    const hasStart = clipStartText.trim() !== "";
    const hasEnd = clipEndText.trim() !== "";
    const start = hasStart ? parseTimecode(clipStartText) : null;
    const end = hasEnd ? parseTimecode(clipEndText) : null;
    if (hasStart && start == null)
      return "Inicio inválido. Usa mm:ss o h:mm:ss.";
    if (hasEnd && end == null) return "Fin inválido. Usa mm:ss o h:mm:ss.";
    if (hasEnd && !hasStart) return "Indica el inicio del recorte.";
    if (start != null && end != null && end <= start)
      return "El fin debe ser mayor que el inicio.";
    return null;
  });

  function syncClip() {
    const start = clipStartText.trim() ? parseTimecode(clipStartText) : null;
    let end = clipEndText.trim() ? parseTimecode(clipEndText) : null;
    if (start != null && end != null && end <= start) end = null;
    options.clipStart = start;
    options.clipEnd = end;
  }

  let heights = $derived(availableHeightsFor(info.heights));
  let resolution1080 = $derived(closestHeight(heights, 1080));
  let showQuality = $derived(qualityApplies(options));
  let reencodes = $derived(
    options.mode === "video" && (options.volumeDb !== 0 || options.normalizeAudio),
  );
  let summary = $derived(summarizeOptions(options));

  function applyPreset(id: PresetId) {
    const height = id === "video-1080" ? resolution1080 : null;
    const preset =
      id === "video-1080"
        ? { values: { mode: "video" as const, videoHeight: height, container: "mp4", embedMetadata: true, embedThumbnail: true } }
        : PRESETS.find((p) => p.id === id);
    if (!preset) return;

    // Reinicia filtros de audio incompatibles con "original"
    Object.assign(options, {
      volumeDb: options.volumeDb ?? 0,
      ...preset.values,
    });
    if (options.mode === "audio" && options.audioFormat === "best") {
      options.volumeDb = 0;
      options.normalizeAudio = false;
    }
    activePreset = id;
  }

  function presetLabel(id: PresetId): string {
    if (id === "video-1080") {
      return resolution1080 ? `Video ${resolution1080}p` : "Video (mejor)";
    }
    return PRESETS.find((p) => p.id === id)?.label ?? id;
  }
</script>

<section class="mt-3 overflow-hidden rounded-xl border border-line bg-surface/30">
  <!-- Pestañas + presets -->
  <div
    class="flex flex-wrap items-center justify-between gap-3 border-b border-line/70 px-4 py-2.5"
  >
    <div class="flex rounded-lg border border-line bg-canvas/60 p-0.5">
      {#each ["video", "audio"] as const as mode (mode)}
        <button
          class="rounded-md px-3 py-1 text-xs font-medium transition-colors {options.mode ===
          mode
            ? 'bg-elevated text-ink'
            : 'text-ink-muted hover:text-ink-strong'}"
          onclick={() => {
            options.mode = mode;
            activePreset = null;
          }}
        >
          {mode === "video" ? "Video" : "Solo audio"}
        </button>
      {/each}
    </div>

    <div class="flex flex-wrap gap-1.5">
      {#each presetIds as id (id)}
        <button
          class="rounded-full border px-2.5 py-1 text-[11px] transition-colors {activePreset ===
          id
            ? 'border-accent/60 bg-accent/10 text-accent-soft'
            : 'border-line text-ink-soft hover:border-line-strong hover:text-ink'}"
          onclick={() => applyPreset(id)}
        >
          {presetLabel(id)}
        </button>
      {/each}
    </div>
  </div>

  <!-- Opciones -->
  <div class="grid grid-cols-2 gap-x-6 gap-y-4 px-4 py-4">
    {#if options.mode === "video"}
      <div>
        <span class="mb-1 block text-[11px] uppercase tracking-wide text-ink-muted"
          >Calidad</span
        >
        <select
          aria-label="Calidad de video"
          bind:value={options.videoHeight}
          class="w-full rounded-lg border border-line bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none focus:border-accent"
        >
          <option value={null}>Mejor disponible</option>
          {#each heights as height (height)}
            <option value={height}>{height}p</option>
          {/each}
        </select>
      </div>

      <div>
        <span class="mb-1 block text-[11px] uppercase tracking-wide text-ink-muted"
          >Contenedor</span
        >
        <select
          aria-label="Contenedor de video"
          bind:value={options.container}
          class="w-full rounded-lg border border-line bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none focus:border-accent"
        >
          {#each VIDEO_CONTAINERS as container (container.id)}
            <option value={container.id}>{container.label}</option>
          {/each}
        </select>
      </div>
    {:else}
      <div>
        <span class="mb-1 block text-[11px] uppercase tracking-wide text-ink-muted"
          >Formato</span
        >
        <select
          aria-label="Formato de audio"
          bind:value={options.audioFormat}
          class="w-full rounded-lg border border-line bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none focus:border-accent"
        >
          {#each AUDIO_FORMATS as format (format.id)}
            <option value={format.id}>{format.label}</option>
          {/each}
        </select>
      </div>

      <div>
        <span class="mb-1 block text-[11px] uppercase tracking-wide text-ink-muted"
          >Calidad</span
        >
        <select
          aria-label="Calidad de audio"
          bind:value={options.audioQuality}
          disabled={!showQuality}
          class="w-full rounded-lg border border-line bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none focus:border-accent disabled:cursor-not-allowed disabled:opacity-40"
        >
          {#each AUDIO_QUALITIES as quality (quality.id)}
            <option value={quality.id}>{quality.label}</option>
          {/each}
        </select>
      </div>
    {/if}

    <!-- Volumen -->
    <div class="col-span-2">
      <div class="mb-1.5 flex items-baseline justify-between">
        <span class="text-[11px] uppercase tracking-wide text-ink-muted"
          >Volumen</span
        >
        <span class="font-mono text-xs text-ink-soft">
          {options.volumeDb === 0
            ? "original"
            : `${options.volumeDb && options.volumeDb > 0 ? "+" : ""}${options.volumeDb} dB`}
        </span>
      </div>
      <div class="flex items-center gap-3">
        <input
          type="range"
          min="-20"
          max="20"
          step="1"
          aria-label="Ajuste de volumen en decibelios"
          disabled={options.audioFormat === "best" && options.mode === "audio"}
          bind:value={options.volumeDb}
          class="h-1 flex-1 cursor-pointer appearance-none rounded-full bg-elevated accent-accent disabled:cursor-not-allowed disabled:opacity-40"
        />
        <button
          class="shrink-0 rounded-md border border-line px-2 py-0.5 text-[11px] text-ink-soft transition-colors hover:bg-surface hover:text-ink"
          onclick={() => (options.volumeDb = 0)}
        >
          Reset
        </button>
      </div>
    </div>

    <!-- Interruptores -->
    <div class="col-span-2 flex flex-wrap items-center gap-x-6 gap-y-2 pt-0.5">
      <label class="flex cursor-pointer items-center gap-2 text-xs text-ink-soft">
        <input
          type="checkbox"
          bind:checked={options.normalizeAudio}
          disabled={options.audioFormat === "best" && options.mode === "audio"}
          class="size-3.5 accent-accent"
        />
        Normalizar volumen
      </label>
      <label class="flex cursor-pointer items-center gap-2 text-xs text-ink-soft">
        <input
          type="checkbox"
          bind:checked={options.embedMetadata}
          class="size-3.5 accent-accent"
        />
        Incrustar metadatos
      </label>
      <label class="flex cursor-pointer items-center gap-2 text-xs text-ink-soft">
        <input
          type="checkbox"
          bind:checked={options.embedThumbnail}
          class="size-3.5 accent-accent"
        />
        Incrustar miniatura
      </label>
    </div>

    <!-- Recorte -->
    <div class="col-span-2 border-t border-line/40 pt-3">
      <div class="mb-1.5 flex items-baseline justify-between">
        <span class="text-[11px] uppercase tracking-wide text-ink-muted"
          >Recorte</span
        >
        {#if options.clipStart != null || clipStartText || clipEndText}
          <button
            class="text-[11px] text-ink-muted transition-colors hover:text-ink-strong"
            onclick={() => {
              clipStartText = "";
              clipEndText = "";
              syncClip();
            }}
          >
            Limpiar
          </button>
        {/if}
      </div>
      <div class="flex flex-wrap items-center gap-2">
        <input
          type="text"
          inputmode="numeric"
          placeholder="Inicio · 0:00"
          aria-label="Inicio del recorte"
          bind:value={clipStartText}
          oninput={syncClip}
          class="w-28 rounded-lg border bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none placeholder:text-ink-faint focus:border-accent {clipError
            ? 'border-danger-bg/60'
            : 'border-line'}"
        />
        <span class="text-xs text-ink-faint">→</span>
        <input
          type="text"
          inputmode="numeric"
          placeholder="Fin · final"
          aria-label="Fin del recorte"
          bind:value={clipEndText}
          oninput={syncClip}
          class="w-28 rounded-lg border bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none placeholder:text-ink-faint focus:border-accent {clipError
            ? 'border-danger-bg/60'
            : 'border-line'}"
        />
        {#if info.duration}
          <span class="text-[11px] text-ink-faint">
            el vídeo dura {formatDuration(info.duration)}
          </span>
        {/if}
      </div>
      {#if clipError}
        <p class="mt-1 text-[11px] text-danger">{clipError}</p>
      {:else if options.clipStart != null}
        <p class="mt-1 text-[11px] text-ink-faint">
          El corte se ajusta al fotograma clave más cercano (puede variar unas
          décimas).
        </p>
      {/if}
    </div>

    <!-- Subtítulos (solo tiene sentido en vídeo) -->
    {#if options.mode === "video"}
      <div class="col-span-2 border-t border-line/40 pt-3">
        <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
          <label
            class="flex cursor-pointer items-center gap-2 text-[11px] uppercase tracking-wide text-ink-muted"
          >
            <input
              type="checkbox"
              bind:checked={options.subtitles}
              class="size-3.5 accent-accent"
            />
            Subtítulos
          </label>
          {#if options.subtitles}
            <input
              type="text"
              placeholder="idiomas: es,en"
              aria-label="Idiomas de subtítulos"
              bind:value={options.subtitleLangs}
              class="w-36 rounded-lg border border-line bg-canvas/60 px-2.5 py-1.5 text-sm text-ink outline-none placeholder:text-ink-faint focus:border-accent"
            />
            <label
              class="flex cursor-pointer items-center gap-2 text-xs text-ink-soft"
            >
              <input
                type="checkbox"
                bind:checked={options.subtitleAuto}
                class="size-3.5 accent-accent"
              />
              Automáticos
            </label>
            <label
              class="flex cursor-pointer items-center gap-2 text-xs text-ink-soft"
            >
              <input
                type="checkbox"
                bind:checked={options.embedSubtitles}
                class="size-3.5 accent-accent"
              />
              Incrustar en el vídeo
            </label>
          {/if}
        </div>
      </div>
    {/if}

    <!-- SponsorBlock (solo YouTube) -->
    <div class="col-span-2 border-t border-line/40 pt-3">
      <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
        <label
          class="flex cursor-pointer items-center gap-2 text-[11px] uppercase tracking-wide text-ink-muted"
        >
          <input
            type="checkbox"
            bind:checked={options.sponsorblock}
            class="size-3.5 accent-accent"
          />
          SponsorBlock
        </label>
        {#if options.sponsorblock}
          <div class="flex rounded-lg border border-line bg-canvas/60 p-0.5">
            {#each sponsorblockModes as mode (mode.id)}
              <button
                class="rounded-md px-2.5 py-0.5 text-[11px] transition-colors {options.sponsorblockMode ===
                mode.id
                  ? 'bg-elevated text-ink'
                  : 'text-ink-muted hover:text-ink-strong'}"
                onclick={() => (options.sponsorblockMode = mode.id)}
              >
                {mode.label}
              </button>
            {/each}
          </div>
        {/if}
      </div>

      {#if options.sponsorblock}
        <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1.5">
          {#each SPONSORBLOCK_CATEGORIES as category (category.id)}
            <label
              class="flex cursor-pointer items-center gap-1.5 text-xs text-ink-soft"
              title={category.description}
            >
              <input
                type="checkbox"
                checked={options.sponsorblockCategories.includes(category.id)}
                onchange={() => toggleCategory(category.id)}
                class="size-3.5 accent-accent"
              />
              {category.label}
            </label>
          {/each}
        </div>
        <p class="mt-1.5 text-[11px] text-ink-faint">
          Solo en YouTube. {options.sponsorblockMode === "remove"
            ? "Los segmentos se quitan del archivo."
            : "Los segmentos se marcan como capítulos."}
          {#if options.sponsorblockCategories.length === 0}
            Sin categorías marcadas se quita solo «Patrocinio».
          {/if}
        </p>
      {/if}
    </div>

    {#if reencodes}
      <p class="col-span-2 text-[11px] text-warn/90">
        El ajuste de volumen en video requiere re-codificar (más lento y con leve
        pérdida de calidad).
      </p>
    {:else if options.mode === "video" && options.container === "mp4"}
      <p class="col-span-2 text-[11px] text-ink-faint">
        En MP4 se prefiere H.264 + AAC para máxima compatibilidad de reproductores.
      </p>
    {/if}
  </div>

  <!-- Pie: resumen + descargar -->
  <div
    class="flex items-center justify-between gap-4 border-t border-line/70 bg-canvas/40 px-4 py-2.5"
  >
    <span class="truncate text-xs text-ink-muted">{summary}</span>
    <button
      class="flex shrink-0 items-center gap-1.5 rounded-md bg-accent px-3.5 py-1.5 text-sm font-medium text-accent-ink shadow-sm shadow-accent/30 transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
      disabled={starting || disabled}
      onclick={onDownload}
    >
      <svg
        class="size-3.5"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        stroke-width="2"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3"
        />
      </svg>
      {starting ? "Iniciando…" : label}
    </button>
  </div>
</section>
