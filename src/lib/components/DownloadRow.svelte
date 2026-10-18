<script lang="ts">
  import type { JobStatus, QueueJob } from "$lib/types";
  import { formatBytes, formatEta, formatSpeed } from "$lib/utils";
  import { summarizeOptions } from "$lib/options";

  interface Props {
    job: QueueJob;
    /** Es un trabajo pendiente y se puede reordenar arrastrándolo. */
    reorderable: boolean;
    dragging: boolean;
    onPause: (id: number) => void;
    onResume: (id: number) => void;
    onCancel: (id: number) => void;
    onRetry: (id: number) => void;
    onReveal: (job: QueueJob) => void;
    onDragStart: (id: number, e: DragEvent) => void;
    onDragOver: (id: number, e: DragEvent) => void;
    onDrop: (id: number, e: DragEvent) => void;
    onDragEnd: () => void;
  }

  let {
    job,
    reorderable,
    dragging,
    onPause,
    onResume,
    onCancel,
    onRetry,
    onReveal,
    onDragStart,
    onDragOver,
    onDrop,
    onDragEnd,
  }: Props = $props();

  const STATUS_LABEL: Record<JobStatus, string> = {
    queued: "En cola",
    downloading: "Descargando",
    processing: "Procesando",
    paused: "En pausa",
    done: "Completado",
    error: "Error",
    cancelled: "Cancelado",
  };

  /** Nombre del postprocesador que reporta yt-dlp. */
  const PHASE_LABEL: Record<string, string> = {
    ExtractAudio: "Extrayendo el audio",
    Merger: "Mezclando vídeo y audio",
    VideoConvertor: "Convirtiendo el vídeo",
    VideoRemuxer: "Reempaquetando el vídeo",
    Metadata: "Escribiendo los metadatos",
    EmbedThumbnail: "Incrustando la carátula",
    FixupM3u8: "Ajustando el vídeo",
    FixupStretched: "Ajustando la duración",
    MoveFiles: "Guardando el archivo",
  };

  /** Stream que se está bajando (una descarga con mezcla baja dos). */
  const STREAM_LABEL: Record<string, string> = {
    video: "Vídeo",
    audio: "Audio",
  };

  let status = $derived(job.status);
  let percent = $derived(job.percent);
  let subtitle = $derived(summarizeOptions(job.options));
  let showBar = $derived(status === "downloading" || status === "processing");
  /** Sin porcentaje no hay barra que llenar: se muestra en movimiento. */
  let indeterminate = $derived(status === "processing" || percent == null);
  let processingText = $derived(
    (job.phase ? PHASE_LABEL[job.phase] : null) ?? "Procesando el archivo",
  );
  let streamLabel = $derived(job.stream ? (STREAM_LABEL[job.stream] ?? null) : null);
</script>

{#snippet progressBar()}
  <div class="h-1.5 overflow-hidden rounded-full bg-elevated">
    {#if indeterminate}
      <div class="progress-indeterminate h-full w-1/3 rounded-full bg-warn"></div>
    {:else}
      <div
        class="h-full rounded-full bg-accent-hover transition-[width] duration-300"
        style="width: {Math.min(percent ?? 0, 100)}%"
      ></div>
    {/if}
  </div>
{/snippet}

<div
  class="rounded-xl border bg-surface/40 px-4 py-3 transition-colors {dragging
    ? 'border-accent/70 opacity-60'
    : 'border-line'}"
  draggable={reorderable}
  ondragstart={(e) => onDragStart(job.id, e)}
  ondragover={(e) => onDragOver(job.id, e)}
  ondrop={(e) => {
    e.preventDefault();
    e.stopPropagation();
    onDrop(job.id, e);
  }}
  ondragend={onDragEnd}
  role="listitem"
>
  <div class="flex items-center gap-3">
    <!-- Indicador de estado -->
    {#if status === "queued"}
      {#if reorderable}
        <span class="shrink-0 cursor-grab text-ink-faint active:cursor-grabbing" title="Arrastra para reordenar">
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M3.75 9h16.5m-16.5 6.75h16.5" />
          </svg>
        </span>
      {:else}
        <span class="grid size-6 shrink-0 place-items-center text-ink-muted">
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
            <circle cx="12" cy="12" r="9" />
            <path stroke-linecap="round" d="M12 7v5l3 2" />
          </svg>
        </span>
      {/if}
    {:else if status === "paused"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-elevated text-ink-strong">
        <svg class="size-3" fill="currentColor" viewBox="0 0 24 24">
          <rect x="6" y="5" width="4" height="14" rx="1" />
          <rect x="14" y="5" width="4" height="14" rx="1" />
        </svg>
      </span>
    {:else if status === "done"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-ok/10 text-ok">
        <svg class="size-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5" />
        </svg>
      </span>
    {:else if status === "error"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-danger/10 text-danger">
        <svg class="size-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
        </svg>
      </span>
    {:else if status === "cancelled"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-elevated text-ink-soft">
        <svg class="size-3" fill="currentColor" viewBox="0 0 24 24">
          <rect x="6" y="6" width="12" height="12" rx="2" />
        </svg>
      </span>
    {:else}
      <span class="grid size-6 shrink-0 place-items-center text-accent-soft">
        <svg class="size-4 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
          <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8v3a5 5 0 0 0-5 5H4z" />
        </svg>
      </span>
    {/if}

    <!-- Título y estado -->
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm text-ink">{job.title}</p>
      <p class="mt-0.5 truncate text-[11px] text-ink-faint">{subtitle}</p>

      {#if status === "downloading"}
        <p class="mt-0.5 text-xs text-ink-muted">
          {#if streamLabel}
            <span
              class="mr-1 rounded bg-elevated px-1 py-px text-[10px] font-medium text-ink-soft"
              >{streamLabel}</span
            >
          {/if}
          {#if percent != null}
            <span class="tabular-nums text-ink-soft">{percent.toFixed(1)}%</span>
            ·
          {/if}
          {formatBytes(job.downloadedBytes)}{#if job.totalBytes}
            de {formatBytes(job.totalBytes)}{/if}
          · {formatSpeed(job.speedBps)} · ETA {formatEta(job.etaSecs)}
        </p>
      {:else if status === "processing"}
        <p class="mt-0.5 text-xs text-ink-muted">{processingText}…</p>
      {:else if status === "queued"}
        <p class="mt-0.5 text-xs text-ink-muted">{STATUS_LABEL.queued}</p>
      {:else if status === "paused"}
        <p class="mt-0.5 text-xs text-ink-muted">
          En pausa — reanuda desde donde iba
        </p>
      {:else if status === "done" && job.message}
        <p class="copyable mt-0.5 whitespace-pre-wrap text-xs text-warn/90"
          >{job.message}</p
        >
      {:else if status === "done" && job.filePath}
        <p class="copyable mt-0.5 truncate font-mono text-xs text-ink-faint">
          {job.filePath}
        </p>
      {:else if status === "error"}
        <p
          class="copyable mt-0.5 whitespace-pre-wrap text-xs text-danger/90"
        >{job.message ?? "Error desconocido"}</p>
      {:else if status === "cancelled"}
        <p class="mt-0.5 text-xs text-ink-muted">Cancelado</p>
      {/if}
    </div>

    <!-- Progreso / acciones -->
    <div class="flex w-44 shrink-0 items-center justify-end gap-1.5 max-md:w-auto">
      {#if showBar}
        <!-- En móvil la barra se va debajo del título (ver más abajo): aquí solo
             hay ancho para los botones. -->
        <div class="mr-1 hidden flex-1 md:block">{@render progressBar()}</div>
      {/if}

      {#if status === "downloading" || status === "processing"}
        <button
          class="rounded-md p-1 text-ink-muted transition-colors tap tap-center hover:bg-elevated hover:text-ink"
          title="Pausar"
          aria-label="Pausar descarga"
          onclick={() => onPause(job.id)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M10 5v14M14 5v14" />
          </svg>
        </button>
      {/if}

      {#if status === "paused"}
        <button
          class="rounded-md p-1 text-ok transition-colors tap tap-center hover:bg-elevated"
          title="Reanudar"
          aria-label="Reanudar descarga"
          onclick={() => onResume(job.id)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 4l14 8-14 8V4z" />
          </svg>
        </button>
      {/if}

      {#if status === "done" && job.filePath}
        <button
          class="rounded-md p-1 text-ink-muted transition-colors tap tap-center hover:bg-elevated hover:text-ink"
          title="Mostrar en la carpeta"
          aria-label="Mostrar el archivo en la carpeta"
          onclick={() => onReveal(job)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
            <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
          </svg>
        </button>
      {/if}

      {#if status === "error" || status === "cancelled"}
        <button
          class="rounded-md p-1 text-ink-muted transition-colors tap tap-center hover:bg-elevated hover:text-accent"
          title="Reintentar"
          aria-label="Reintentar descarga"
          onclick={() => onRetry(job.id)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M16.023 9.348h4.992v-.001M2.985 19.644v-4.992m0 0h4.992m-4.993 0 3.181 3.183a8.25 8.25 0 0 0 13.803-3.7M4.031 9.865a8.25 8.25 0 0 1 13.803-3.7l3.181 3.182m0-4.991v4.99" />
          </svg>
        </button>
      {/if}

      {#if status !== "done"}
        <button
          class="rounded-md p-1 text-ink-muted transition-colors tap tap-center hover:bg-elevated hover:text-ink-strong"
          title={status === "error" || status === "cancelled"
            ? "Quitar de la lista"
            : status === "queued" || status === "paused"
              ? "Quitar de la cola"
              : "Cancelar"}
          aria-label={status === "error" || status === "cancelled"
            ? "Quitar de la lista"
            : "Cancelar descarga"}
          onclick={() => onCancel(job.id)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      {/if}
    </div>
  </div>

  <!-- En móvil el progreso va a lo ancho, debajo de la fila: junto a los botones
       (44 px cada uno) no cabría. -->
  {#if showBar}
    <div class="mt-2.5 md:hidden">{@render progressBar()}</div>
  {/if}
</div>
