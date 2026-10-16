<script lang="ts">
  import { onMount } from "svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import {
    cancelDownload,
    clearFinished,
    getQueue,
    onQueueUpdated,
    pauseDownload,
    probeUrl,
    reorderQueue,
    resumeDownload,
    startDownload,
  } from "$lib/api";
  import type { ProbeInfo, QueueJob } from "$lib/types";
  import PreviewCard from "$lib/components/PreviewCard.svelte";
  import OptionsPanel from "$lib/components/OptionsPanel.svelte";
  import PlaylistPanel from "$lib/components/PlaylistPanel.svelte";
  import QueueList from "$lib/components/QueueList.svelte";
  import { defaultOptions } from "$lib/options";
  import type { DownloadOptions } from "$lib/types";

  interface Props {
    onGoToSettings: () => void;
  }

  let { onGoToSettings }: Props = $props();

  let url = $state("");
  let probing = $state(false);
  let errorMessage = $state<string | null>(null);
  let probe = $state<ProbeInfo | null>(null);
  let jobs = $state<QueueJob[]>([]);
  let starting = $state(false);
  let options = $state<DownloadOptions>(defaultOptions());
  let adding = $state(0);
  let dropActive = $state(false);
  let selectedIndices = $state<Set<number>>(new Set());

  const STATUS_ORDER: Record<QueueJob["status"], number> = {
    downloading: 0,
    processing: 1,
    queued: 2,
    paused: 3,
    error: 4,
    done: 5,
    cancelled: 6,
  };

  let orderedJobs = $derived.by(() => {
    const index = new Map(jobs.map((j, i) => [j.id, i] as const));
    return [...jobs].sort((a, b) => {
      const diff = STATUS_ORDER[a.status] - STATUS_ORDER[b.status];
      if (diff !== 0) return diff;
      // Los pendientes conservan el orden de despacho que mantiene el backend.
      if (a.status === "queued") {
        return (index.get(a.id) ?? 0) - (index.get(b.id) ?? 0);
      }
      return b.id - a.id;
    });
  });

  let needsCookies = $derived(
    !!errorMessage && /cookies|anti-bot|verificaci|sign in/i.test(errorMessage),
  );

  let downloadLabel = $derived(
    probe?.kind === "playlist"
      ? `Descargar ${selectedIndices.size} vídeo${selectedIndices.size === 1 ? "" : "s"}`
      : "Descargar",
  );

  function extractError(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return "Error inesperado";
  }

  function extractUrls(text: string): string[] {
    const matches = text.match(/https?:\/\/[^\s"'<>]+/gi) ?? [];
    return [...new Set(matches.map((m) => m.trim()))];
  }

  async function analyze(raw?: string) {
    const target = (raw ?? url).trim();
    if (!target || probing) return;
    url = target;
    probing = true;
    errorMessage = null;
    probe = null;
    try {
      probe = await probeUrl(target);
      selectedIndices =
        probe.kind === "playlist"
          ? new Set(probe.entries.map((_, index) => index))
          : new Set();
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      probing = false;
    }
  }

  /** Un enlace → previsualiza; varios → van directos a la cola. */
  async function handleIncoming(text: string) {
    const urls = extractUrls(text);
    if (urls.length === 0) return;
    if (urls.length === 1) {
      await analyze(urls[0]);
      return;
    }
    await enqueueUrls(urls);
  }

  async function enqueueUrls(urls: string[]) {
    const snapshot = $state.snapshot(options);
    adding = urls.length;
    for (const link of urls) {
      try {
        const info = await probeUrl(link);
        if (info.kind === "playlist") {
          for (const entry of info.entries) {
            await startDownload(entry.url, entry.title, snapshot, entry.thumbnail);
          }
        } else {
          await startDownload(info.webpageUrl, info.title, snapshot, info.thumbnail);
        }
      } catch (e) {
        errorMessage = extractError(e);
      } finally {
        adding = Math.max(0, adding - 1);
      }
    }
    adding = 0;
  }

  async function download() {
    if (!probe || starting) return;

    const targets =
      probe.kind === "playlist"
        ? probe.entries
            .filter((_, index) => selectedIndices.has(index))
            .map((entry) => ({
              url: entry.url,
              title: entry.title,
              thumbnail: entry.thumbnail,
            }))
        : [
            {
              url: probe.webpageUrl,
              title: probe.title,
              thumbnail: probe.thumbnail,
            },
          ];

    if (targets.length === 0) return;

    starting = true;
    errorMessage = null;
    const snapshot = $state.snapshot(options);
    try {
      for (const target of targets) {
        await startDownload(
          target.url,
          target.title,
          snapshot,
          target.thumbnail,
        );
      }
      probe = null;
      url = "";
      selectedIndices = new Set();
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      starting = false;
    }
  }

  function toggleEntry(index: number) {
    const next = new Set(selectedIndices);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    selectedIndices = next;
  }

  function selectAllEntries() {
    selectedIndices = new Set(probe?.entries.map((_, index) => index) ?? []);
  }

  function selectNoEntries() {
    selectedIndices = new Set();
  }

  async function pause(id: number) {
    try {
      await pauseDownload(id);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function resume(id: number) {
    try {
      await resumeDownload(id);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function cancel(id: number) {
    try {
      await cancelDownload(id);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function reorder(ids: number[]) {
    try {
      await reorderQueue(ids);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function reveal(job: QueueJob) {
    if (!job.filePath) return;
    try {
      await revealItemInDir(job.filePath);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  async function clear() {
    try {
      await clearFinished();
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  let dragDepth = 0;

  function isExternalDrag(e: DragEvent): boolean {
    const types = e.dataTransfer?.types;
    return (
      !!types &&
      (types.includes("text/uri-list") ||
        types.includes("text/plain") ||
        types.includes("Files"))
    );
  }

  function onDragEnter(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    dragDepth += 1;
    dropActive = true;
  }

  function onDragLeave() {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) dropActive = false;
  }

  function onDragOver(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  }

  function onDrop(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    e.preventDefault();
    dragDepth = 0;
    dropActive = false;
    const text =
      e.dataTransfer?.getData("text/uri-list") ||
      e.dataTransfer?.getData("text/plain") ||
      "";
    handleIncoming(text);
  }

  onMount(() => {
    const unlistenQueue = onQueueUpdated((list) => {
      jobs = list;
    });
    getQueue().then((list) => {
      jobs = list;
    });

    const onPaste = (e: ClipboardEvent) => {
      const text = e.clipboardData?.getData("text")?.trim();
      if (text && /^https?:\/\//i.test(text)) handleIncoming(text);
    };

    window.addEventListener("paste", onPaste);
    window.addEventListener("dragenter", onDragEnter);
    window.addEventListener("dragleave", onDragLeave);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);

    return () => {
      unlistenQueue.then((un) => un());
      window.removeEventListener("paste", onPaste);
      window.removeEventListener("dragenter", onDragEnter);
      window.removeEventListener("dragleave", onDragLeave);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
    };
  });
</script>

<div class="relative mx-auto max-w-3xl px-10 py-12">
  <header class="mb-8">
    <h1 class="text-lg font-semibold tracking-tight text-ink">Descargar</h1>
    <p class="mt-1 text-sm text-ink-muted">
      Pega o arrastra enlaces de YouTube, TikTok, Instagram y +1800 sitios
      soportados
    </p>
  </header>

  <!-- Input de URL -->
  <div class="relative">
    <svg
      class="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-ink-muted"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      stroke-width="2"
    >
      <path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 0 1 1.242 7.244l-4.5 4.5a4.5 4.5 0 0 1-6.364-6.364l1.757-1.757m13.35-.687 1.757-1.757a4.5 4.5 0 0 0-6.364-6.364l-4.5 4.5a4.5 4.5 0 0 0 1.242 7.244" />
    </svg>
    <input
      type="url"
      bind:value={url}
      placeholder="https://www.youtube.com/watch?v=..."
      class="w-full rounded-lg border border-line bg-surface/50 py-3 pl-10 pr-28 text-sm text-ink outline-none transition-colors placeholder:text-ink-faint focus:border-accent"
      onkeydown={(e) => {
        if (e.key === "Enter") analyze();
      }}
    />
    <button
      class="absolute right-1.5 top-1.5 bottom-1.5 rounded-md bg-accent px-4 text-sm font-medium text-accent-ink shadow-sm shadow-accent/30 transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
      disabled={probing || !url.trim()}
      onclick={() => analyze()}
    >
      {#if probing}
        <span class="flex items-center gap-2">
          <svg class="size-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
            <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8v3a5 5 0 0 0-5 5H4z" />
          </svg>
          Analizando…
        </span>
      {:else}
        Analizar
      {/if}
    </button>
  </div>

  <p class="mt-2 text-xs text-ink-faint">
    Tip: pega uno o varios enlaces con <span class="font-mono text-ink-muted">Ctrl+V</span>,
    o arrástralos directamente a la ventana
  </p>

  <!-- Enlaces añadiéndose a la cola -->
  {#if adding > 0}
    <div
      class="mt-4 flex items-center gap-2.5 rounded-lg border border-line bg-surface/30 px-4 py-3 text-sm text-ink-soft"
    >
      <svg class="size-4 animate-spin text-accent-soft" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
        <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8v3a5 5 0 0 0-5 5H4z" />
      </svg>
      Añadiendo {adding} enlace{adding === 1 ? "" : "s"} a la cola…
    </div>
  {/if}

  <!-- Error -->
  {#if errorMessage}
    <div
      class="mt-4 rounded-lg border border-danger-bg/50 bg-danger-bg/30 px-4 py-3 text-sm text-danger"
    >
      <p class="whitespace-pre-line">{errorMessage}</p>
      {#if needsCookies}
        <button
          class="mt-2.5 rounded-md bg-danger-bg/40 px-3 py-1.5 text-xs font-medium text-danger transition-colors hover:bg-danger-bg/60"
          onclick={onGoToSettings}
        >
          Configurar cookies en Ajustes →
        </button>
      {/if}
    </div>
  {/if}

  <!-- Preview del enlace analizado -->
  {#if probe}
    <div class="mt-6">
      <PreviewCard info={probe} />
      {#if probe.kind === "playlist"}
        <PlaylistPanel
          entries={probe.entries}
          selected={selectedIndices}
          truncated={probe.truncated}
          onToggle={toggleEntry}
          onAll={selectAllEntries}
          onNone={selectNoEntries}
        />
      {/if}
      <OptionsPanel
        info={probe}
        {options}
        onDownload={download}
        {starting}
        label={downloadLabel}
        disabled={probe.kind === "playlist" && selectedIndices.size === 0}
      />
    </div>
  {/if}

  <!-- Cola -->
  <QueueList
    jobs={orderedJobs}
    onPause={pause}
    onResume={resume}
    onCancel={cancel}
    onReveal={reveal}
    onReorder={reorder}
    onClearFinished={clear}
  />

  <!-- Resaltado al arrastrar enlaces -->
  {#if dropActive}
    <div
      class="pointer-events-none fixed inset-4 z-50 rounded-2xl border-2 border-dashed border-accent-hover/70 bg-accent-hover/5"
    >
      <div class="absolute inset-x-0 top-6 text-center text-sm font-medium text-accent-soft">
        Suelta para añadir a la cola
      </div>
    </div>
  {/if}
</div>
