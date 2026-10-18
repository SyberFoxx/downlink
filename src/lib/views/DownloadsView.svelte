<script lang="ts">
  import {
    analyze,
    cancel,
    clear,
    download,
    pause,
    reorder,
    resume,
    retry,
    reveal,
    downloads,
  } from "$lib/downloads.svelte";
  import type { QueueJob } from "$lib/types";
  import PreviewCard from "$lib/components/PreviewCard.svelte";
  import OptionsPanel from "$lib/components/OptionsPanel.svelte";
  import PlaylistPanel from "$lib/components/PlaylistPanel.svelte";
  import QueueList from "$lib/components/QueueList.svelte";

  interface Props {
    onGoToSettings: () => void;
  }

  let { onGoToSettings }: Props = $props();

  // El estado (enlace, probe, opciones, cola) vive en `$lib/downloads.svelte`
  // para sobrevivir a los cambios de vista; aquí solo se lee.
  let probe = $derived(downloads.probe);
  let errorMessage = $derived(downloads.errorMessage);
  let adding = $derived(downloads.adding);
  let probing = $derived(downloads.probing);
  let starting = $derived(downloads.starting);
  let jobs = $derived(downloads.jobs);
  let selectedIndices = $derived(downloads.selectedIndices);

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

  function toggleEntry(index: number) {
    const next = new Set(selectedIndices);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    downloads.selectedIndices = next;
  }

  function selectAllEntries() {
    downloads.selectedIndices = new Set(
      probe?.entries.map((_, index) => index) ?? [],
    );
  }

  function selectNoEntries() {
    downloads.selectedIndices = new Set();
  }
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
      aria-label="Enlace del vídeo a descargar"
      bind:value={downloads.url}
      placeholder="https://www.youtube.com/watch?v=..."
      class="tap w-full rounded-lg border border-line bg-surface/50 py-3 pl-10 pr-28 text-sm text-ink outline-none transition-colors placeholder:text-ink-faint focus:border-accent"
      onkeydown={(e) => {
        if (e.key === "Enter") analyze();
      }}
      onpaste={(e) => {
        // Al pegar en el propio campo se analiza al momento (el escuchador
        // global ignora los campos de texto para no interferir con ellos).
        const text = e.clipboardData?.getData("text")?.trim();
        if (text && /^https?:\/\//i.test(text)) void analyze(text);
      }}
    />
    <button
      class="tap tap-center absolute right-1.5 top-1.5 bottom-1.5 max-md:top-1/2 max-md:bottom-auto max-md:-translate-y-1/2 rounded-md bg-accent px-4 text-sm font-medium text-accent-ink shadow-sm shadow-accent/30 transition-colors hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-50"
      disabled={probing || !downloads.url.trim()}
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
      role="alert"
    >
      <div class="flex items-start justify-between gap-3">
        <p class="copyable whitespace-pre-line">{errorMessage}</p>
        <button
          class="tap tap-center -mr-1 -mt-0.5 shrink-0 rounded-md p-1 text-danger/70 transition-colors hover:bg-danger-bg/40 hover:text-danger"
          aria-label="Cerrar el aviso de error"
          onclick={() => (downloads.errorMessage = null)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      </div>
      {#if needsCookies}
        <button
          class="tap mt-2.5 rounded-md bg-danger-bg/40 px-3 py-1.5 text-xs font-medium text-danger transition-colors hover:bg-danger-bg/60"
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
        options={downloads.options}
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
    loading={!downloads.queueLoaded}
    onPause={pause}
    onResume={resume}
    onCancel={cancel}
    onRetry={retry}
    onReveal={reveal}
    onReorder={reorder}
    onClearFinished={clear}
  />
</div>
