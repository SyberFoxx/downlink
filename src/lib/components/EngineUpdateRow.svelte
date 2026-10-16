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

<div class="mt-3 rounded-lg border border-gray-800 bg-gray-900/30 px-3.5 py-3">
  <div class="flex items-center justify-between gap-3">
    <div class="min-w-0">
      {#if updating && progress}
        <p class="text-sm text-violet-300">{label}</p>
      {:else if error}
        <p class="text-sm text-red-400">{error}</p>
      {:else if info?.updateAvailable && info.current === null && missingLabel}
        <p class="text-sm text-amber-400">{missingLabel}</p>
      {:else if info?.updateAvailable}
        <p class="text-sm text-amber-400">
          Nueva versión disponible: {info.latest}
        </p>
      {:else if info}
        <p class="text-sm text-gray-400">{name} está al día</p>
      {:else}
        <p class="text-sm text-gray-400">{idleLabel}</p>
      {/if}
    </div>
    <div class="flex shrink-0 gap-2">
      <button
        class="rounded-md border border-gray-800 px-3 py-1.5 text-xs text-gray-300 transition-colors hover:bg-gray-900 disabled:opacity-50"
        disabled={checking || updating}
        onclick={refresh}
      >
        {checking ? "Buscando…" : "Buscar"}
      </button>
      {#if info?.updateAvailable}
        <button
          class="rounded-md bg-violet-600 px-3 py-1.5 text-xs font-medium text-white transition-colors hover:bg-violet-500 disabled:opacity-50"
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
    </div>
  </div>

  {#if updating && percent !== null}
    <div class="mt-2.5 h-1 overflow-hidden rounded-full bg-gray-800">
      <div
        class="h-full rounded-full bg-violet-600 transition-[width] duration-200"
        style="width: {percent}%"
      ></div>
    </div>
  {/if}

  <label class="mt-3 flex items-center gap-2">
    <input
      type="checkbox"
      bind:checked={autoChecked}
      class="size-3.5 accent-violet-600"
    />
    <span class="text-xs text-gray-400">{autoLabel}</span>
  </label>

  {#if hint}
    <p class="mt-1.5 text-[11px] text-gray-600">{hint}</p>
  {/if}
</div>
