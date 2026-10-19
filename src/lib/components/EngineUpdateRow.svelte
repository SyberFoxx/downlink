<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { onEngineUpdate } from "$lib/api";
  import type {
    EngineUpdatePhase,
    EngineUpdateProgress,
    UpdateCheck,
  } from "$lib/types";

  const PHASE_LABELS: Record<EngineUpdatePhase, string> = {
    checking: "Buscando actualización…",
    downloading: "Descargando…",
    verifying: "Verificando integridad…",
    installing: "Instalando…",
    done: "Actualizado ✓",
    error: "Error",
  };

  let {
    engine,
    name,
    idleLabel,
    missingLabel = null,
    autoLabel,
    hint = null,
    bundled = false,
    autoChecked = $bindable(true),
    check,
    update,
    onChanged,
  }: {
    /** Identificador del motor tal cual viaja en el evento `engine-update`. */
    engine: string;
    /** Nombre para los textos ("yt-dlp", "ffmpeg"). */
    name: string;
    /** Texto cuando todavía no se ha comprobado nada. */
    idleLabel: string;
    /** Texto cuando no hay ningún binario disponible (opcional). */
    missingLabel?: string | null;
    autoLabel: string;
    /** Nota al pie para la casilla (opcional). */
    hint?: string | null;
    /**
     * El motor viene incluido con la aplicación (Android): no se descarga ni se
     * actualiza, así que se ocultan los botones y la casilla de auto-descarga.
     */
    bundled?: boolean;
    autoChecked?: boolean;
    check: () => Promise<UpdateCheck>;
    update: () => Promise<UpdateCheck>;
    /** Se llama tras una actualización correcta (para refrescar Ajustes). */
    onChanged?: () => void;
  } = $props();

  let info = $state<UpdateCheck | null>(null);
  let progress = $state<EngineUpdateProgress | null>(null);
  let checking = $state(false);
  let updating = $state(false);
  let error = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  let label = $derived(progress ? PHASE_LABELS[progress.phase] : "");
  let percent = $derived(
    typeof progress?.percent === "number" ? progress.percent : null,
  );

  onMount(async () => {
    unlisten = await onEngineUpdate(handleProgress);
    await refresh();
  });

  onDestroy(() => unlisten?.());

  function handleProgress(next: EngineUpdateProgress) {
    if (next.engine !== engine) return;
    progress = next;
    if (next.phase === "error") {
      error = next.message ?? "La actualización falló";
      updating = false;
    } else if (next.phase === "done") {
      updating = false;
      if (info) {
        info = { ...info, current: info.latest, updateAvailable: false };
      }
      onChanged?.();
    } else {
      updating = true;
    }
  }

  async function refresh() {
    checking = true;
    error = null;
    try {
      info = await check();
    } catch (e) {
      error = typeof e === "string" ? e : "No se pudo comprobar la actualización";
    } finally {
      checking = false;
    }
  }

  async function runUpdate() {
    updating = true;
    error = null;
    progress = null;
    try {
      info = await update();
      onChanged?.();
      setTimeout(() => (progress = null), 2500);
    } catch (e) {
      error = typeof e === "string" ? e : "La actualización falló";
    } finally {
      updating = false;
    }
  }
</script>

<div class="mt-3 rounded-lg border border-line bg-surface/30 px-3.5 py-3">
  <div class="flex items-center justify-between gap-3">
    <div class="min-w-0">
      {#if updating && progress}
        <p class="text-sm text-accent-soft">{label}</p>
      {:else if error}
        <p class="text-sm text-danger">{error}</p>
      {:else if info?.updateAvailable && info.current === null && missingLabel}
        <p class="text-sm text-warn">{missingLabel}</p>
      {:else if info?.updateAvailable}
        <p class="text-sm text-warn">
          Nueva versión disponible: {info.latest}
        </p>
      {:else if info}
        <p class="text-sm text-ink-soft">{name} está al día</p>
      {:else}
        <p class="text-sm text-ink-soft">{idleLabel}</p>
      {/if}
    </div>
    <div class="flex shrink-0 gap-2">
      {#if !bundled}
        <button
          class="tap rounded-md border border-line px-3 py-1.5 text-xs text-ink-strong transition-colors hover:bg-surface disabled:opacity-50"
          disabled={checking || updating}
          onclick={refresh}
        >
          {checking ? "Buscando…" : "Buscar"}
        </button>
        {#if info?.updateAvailable}
          <button
            class="tap rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-accent-ink transition-colors hover:bg-accent-hover disabled:opacity-50"
            disabled={updating}
            onclick={runUpdate}
          >
            {#if updating}
              Actualizando…
            {:else if info.current === null}
              Descargar
            {:else}
              Actualizar
            {/if}
          </button>
        {/if}
      {/if}
    </div>
  </div>

  {#if updating && percent !== null}
    <div class="mt-2.5 h-1 overflow-hidden rounded-full bg-elevated">
      <div
        class="h-full rounded-full bg-accent transition-[width] duration-200"
        style="width: {percent}%"
      ></div>
    </div>
  {/if}

  {#if !bundled}
    <label class="tap mt-3 flex items-center gap-2">
      <input
        type="checkbox"
        bind:checked={autoChecked}
        class="size-3.5 max-md:size-5 accent-accent"
      />
      <span class="text-xs text-ink-soft">{autoLabel}</span>
    </label>
  {/if}

  {#if hint}
    <p class="mt-1.5 text-[11px] text-ink-faint">{hint}</p>
  {/if}
</div>
