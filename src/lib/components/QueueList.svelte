<script lang="ts">
  import type { QueueJob } from "$lib/types";
  import DownloadRow from "$lib/components/DownloadRow.svelte";
  import { QUEUE_DRAG_TYPE } from "$lib/downloads.svelte";

  interface Props {
    jobs: QueueJob[];
    /** El backend aún no ha enviado la primera foto de la cola. */
    loading?: boolean;
    onPause: (id: number) => void;
    onResume: (id: number) => void;
    onCancel: (id: number) => void;
    onRetry: (id: number) => void;
    onReveal: (job: QueueJob) => void;
    onReorder: (ids: number[]) => void;
    onClearFinished: () => void;
  }

  let {
    jobs,
    loading = false,
    onPause,
    onResume,
    onCancel,
    onRetry,
    onReveal,
    onReorder,
    onClearFinished,
  }: Props = $props();

  let dragId = $state<number | null>(null);

  let active = $derived(
    jobs.filter((j) => j.status !== "done" && j.status !== "error" && j.status !== "cancelled")
      .length,
  );
  let finished = $derived(jobs.length - active);

  function handleDragStart(id: number, e: DragEvent) {
    dragId = id;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", String(id));
      // Marca interna: el arrastre global de enlaces la excluye para no
      // confundir un reordenamiento con un enlace soltado.
      e.dataTransfer.setData(QUEUE_DRAG_TYPE, String(id));
    }
  }

  function handleDragOver(id: number, e: DragEvent) {
    if (dragId === null || dragId === id) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  }

  function handleDrop(targetId: number) {
    const sourceId = dragId;
    dragId = null;
    if (sourceId === null || sourceId === targetId) return;

    const order = jobs.filter((j) => j.status === "queued").map((j) => j.id);
    const from = order.indexOf(sourceId);
    const to = order.indexOf(targetId);
    if (from === -1 || to === -1) return;

    order.splice(to, 0, ...order.splice(from, 1));
    onReorder(order);
  }

  function handleDragEnd() {
    dragId = null;
  }
</script>

<section class="mt-10">
  <div class="mb-3 flex items-baseline justify-between gap-3">
    <div class="flex items-baseline gap-2">
      <h2 class="text-sm font-medium text-ink-strong">Cola de descargas</h2>
      {#if jobs.length > 0}
        <span class="text-xs text-ink-faint">
          {#if active > 0}
            {active} activa{active === 1 ? "" : "s"}
          {/if}
          {#if active > 0 && finished > 0}·{/if}
          {#if finished > 0}
            {finished} terminada{finished === 1 ? "" : "s"}
          {/if}
        </span>
      {/if}
    </div>

    {#if finished > 0}
      <button
        class="tap shrink-0 rounded-md border border-line px-2.5 py-1 text-[11px] text-ink-muted transition-colors hover:border-line-strong hover:text-ink-strong"
        onclick={onClearFinished}
      >
        Limpiar completadas
      </button>
    {/if}
  </div>

  {#if loading}
    <div class="space-y-2" aria-hidden="true">
      {#each [0, 1] as i (i)}
        <div class="rounded-xl border border-line bg-surface/40 px-4 py-3">
          <div class="flex items-center gap-3">
            <div class="size-6 shrink-0 animate-pulse rounded-full bg-elevated"></div>
            <div class="min-w-0 flex-1 space-y-1.5">
              <div class="h-3.5 w-1/2 animate-pulse rounded bg-elevated"></div>
              <div class="h-2.5 w-1/3 animate-pulse rounded bg-elevated"></div>
            </div>
            <div class="flex w-44 shrink-0 justify-end max-md:w-auto">
              <div class="size-6 animate-pulse rounded-md bg-elevated"></div>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {:else if jobs.length === 0}
    <div class="rounded-xl border border-dashed border-line px-6 py-10 text-center">
      <p class="text-sm text-ink-muted">Todavía no hay nada en la cola</p>
      <p class="mt-1 text-xs text-ink-faint">
        Pega enlaces con Ctrl+V o arrástralos a la ventana
      </p>
    </div>
  {:else}
    <div class="space-y-2" role="list">
      {#each jobs as job (job.id)}
        <DownloadRow
          {job}
          reorderable={job.status === "queued"}
          dragging={dragId === job.id}
          {onPause}
          {onResume}
          {onCancel}
          {onRetry}
          {onReveal}
          onDragStart={handleDragStart}
          onDragOver={handleDragOver}
          onDrop={handleDrop}
          onDragEnd={handleDragEnd}
        />
      {/each}
    </div>
  {/if}
</section>
