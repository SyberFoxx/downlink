<script lang="ts">
  import { onMount } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    clearHistory,
    getHistory,
    removeHistory,
    startDownload,
  } from "$lib/api";
  import { summarizeOptions } from "$lib/options";
  import type { HistoryEntry } from "$lib/types";
  import { formatBytes, formatDate } from "$lib/utils";

  interface Props {
    onGoToDownloads: () => void;
  }

  let { onGoToDownloads }: Props = $props();

  let entries = $state<HistoryEntry[]>([]);
  let query = $state("");
  let loading = $state(true);
  let errorMessage = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let confirmingClear = $state(false);

  onMount(load);

  async function load() {
    loading = true;
    try {
      entries = await getHistory();
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      loading = false;
    }
  }

  let filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter(
      (entry) =>
        entry.title.toLowerCase().includes(q) ||
        entry.url.toLowerCase().includes(q),
    );
  });

  function extractError(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return "Error inesperado";
  }

  async function reveal(entry: HistoryEntry) {
    if (!entry.filePath) return;
    try {
      await revealItemInDir(entry.filePath);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function redownload(entry: HistoryEntry) {
    busy = entry.url;
    errorMessage = null;
    try {
      await startDownload(
        entry.url,
        entry.title,
        entry.options,
        entry.thumbnail,
      );
      onGoToDownloads();
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      busy = null;
    }
  }

  async function remove(entry: HistoryEntry) {
    try {
      await removeHistory(entry.url);
      entries = entries.filter((item) => item.url !== entry.url);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function clearAll() {
    try {
      await clearHistory();
      entries = [];
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  /** Confirmación en dos pasos (sin diálogos nativos). */
  function requestClear() {
    if (!confirmingClear) {
      confirmingClear = true;
      setTimeout(() => (confirmingClear = false), 3000);
      return;
    }
    confirmingClear = false;
    void clearAll();
  }
</script>

<div class="mx-auto max-w-5xl px-10 py-12">
  <header class="mb-8 flex items-end justify-between gap-4">
    <div>
      <h1 class="text-lg font-semibold tracking-tight text-ink">Historial</h1>
      <p class="mt-1 text-sm text-ink-muted">
        Tus descargas completadas, con búsqueda y re-descarga
      </p>
    </div>
    {#if entries.length > 0}
      <button
        class="shrink-0 rounded-md border px-3 py-1.5 text-xs transition-colors {confirmingClear
          ? 'border-danger-bg/60 text-danger'
          : 'border-line text-ink-soft hover:border-danger-bg/60 hover:text-danger'}"
        onclick={requestClear}
      >
        {confirmingClear ? "¿Seguro? Pulsa otra vez" : "Vaciar historial"}
      </button>
    {/if}
  </header>

  {#if entries.length > 0}
    <div class="relative mb-6">
      <svg
        class="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-ink-muted"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        stroke-width="2"
      >
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          d="m21 21-5.197-5.197m0 0A7.5 7.5 0 1 0 5.196 5.196a7.5 7.5 0 0 0 10.607 10.607Z"
        />
      </svg>
      <input
        type="text"
        bind:value={query}
        placeholder="Buscar por título o enlace…"
        class="w-full rounded-lg border border-line bg-surface/50 py-2.5 pl-10 pr-4 text-sm text-ink outline-none transition-colors placeholder:text-ink-faint focus:border-accent"
      />
    </div>
  {/if}

  {#if errorMessage}
    <div
      class="mb-6 rounded-lg border border-danger-bg/50 bg-danger-bg/30 px-4 py-3 text-sm text-danger"
    >
      <p class="whitespace-pre-line">{errorMessage}</p>
    </div>
  {/if}

  {#if loading}
    <p class="text-sm text-ink-muted">Cargando historial…</p>
  {:else if entries.length === 0}
    <div
      class="rounded-xl border border-dashed border-line px-6 py-16 text-center"
    >
      <p class="text-sm text-ink-soft">Todavía no has descargado nada</p>
      <p class="mt-1 text-xs text-ink-faint">
        Las descargas completadas aparecerán aquí automáticamente
      </p>
    </div>
  {:else if filtered.length === 0}
    <p class="text-sm text-ink-muted">Sin resultados para «{query}»</p>
  {:else}
    <p class="mb-3 text-xs text-ink-faint">
      {filtered.length} de {entries.length} descargas
    </p>
    <div class="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {#each filtered as entry (entry.url)}
        <article
          class="group flex flex-col overflow-hidden rounded-xl border border-line bg-surface/40 transition-colors hover:border-line-strong"
        >
          <div class="relative aspect-video bg-surface">
            {#if entry.thumbnail}
              <img
                src={entry.thumbnail}
                alt=""
                loading="lazy"
                class="size-full object-cover"
              />
            {:else}
              <div class="grid size-full place-items-center text-ink-faint">
                <svg class="size-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                  <path stroke-linecap="round" stroke-linejoin="round" d="m15.75 10.5 4.72-4.72a.75.75 0 0 1 1.28.53v11.38a.75.75 0 0 1-1.28.53l-4.72-4.72M4.5 18.75h9a2.25 2.25 0 0 0 2.25-2.25v-9a2.25 2.25 0 0 0-2.25-2.25h-9A2.25 2.25 0 0 0 2.25 7.5v9a2.25 2.25 0 0 0 2.25 2.25Z" />
                </svg>
              </div>
            {/if}
            <div class="absolute inset-x-0 bottom-0 flex items-center justify-between gap-2 bg-gradient-to-t from-black/80 to-transparent px-2.5 pb-1.5 pt-6 text-[11px] text-ink">
              <span class="truncate">{summarizeOptions(entry.options)}</span>
              {#if entry.sizeBytes}
                <span class="shrink-0 tabular-nums text-ink-strong">
                  {formatBytes(entry.sizeBytes)}
                </span>
              {/if}
            </div>
          </div>

          <div class="flex min-h-0 flex-1 flex-col p-3">
            <p class="line-clamp-2 text-sm font-medium leading-snug text-ink">
              {entry.title}
            </p>
            <p class="mt-1 text-[11px] text-ink-muted">{formatDate(entry.finishedAt)}</p>

            <div class="mt-auto flex items-center gap-1.5 pt-3">
              <button
                class="flex-1 rounded-md bg-accent px-2.5 py-1.5 text-xs font-medium text-accent-ink transition-colors hover:bg-accent-hover disabled:opacity-50"
                disabled={busy === entry.url}
                onclick={() => redownload(entry)}
              >
                {busy === entry.url ? "Añadiendo…" : "Re-descargar"}
              </button>
              <button
                class="rounded-md border border-line p-1.5 text-ink-soft transition-colors hover:border-line-strong hover:text-ink disabled:opacity-40"
                title="Mostrar en la carpeta"
                disabled={!entry.filePath}
                onclick={() => reveal(entry)}
              >
                <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.7">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
                </svg>
              </button>
              <button
                class="rounded-md border border-line p-1.5 text-ink-soft transition-colors hover:border-danger-bg/60 hover:text-danger"
                title="Quitar del historial"
                onclick={() => remove(entry)}
              >
                <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.7">
                  <path stroke-linecap="round" stroke-linejoin="round" d="m14.74 9-.346 9m-4.788 0L9.26 9m9.968-3.21c.342.052.682.107 1.022.166m-1.022-.165L18.16 19.673a2.25 2.25 0 0 1-2.244 2.077H8.084a2.25 2.25 0 0 1-2.244-2.077L4.772 5.79m14.456 0a48.108 48.108 0 0 0-3.478-.397m-12 .562c.34-.059.68-.114 1.022-.165m0 0a48.11 48.11 0 0 1 3.478-.397m7.5 0v-.916c0-1.18-.91-2.164-2.09-2.201a51.964 51.964 0 0 0-3.32 0c-1.18.037-2.09 1.022-2.09 2.201v.916m7.5 0a48.667 48.667 0 0 0-7.5 0" />
                </svg>
              </button>
            </div>
          </div>
        </article>
      {/each}
    </div>
  {/if}
</div>
