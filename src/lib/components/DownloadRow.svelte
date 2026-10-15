<script lang="ts">
  import type { DownloadProgress } from "$lib/types";
  import { formatBytes, formatEta, formatSpeed } from "$lib/utils";

  interface Job {
    id: number;
    title: string;
    subtitle: string;
    progress: DownloadProgress | null;
  }

  interface Props {
    job: Job;
    onCancel: (id: number) => void;
  }

  let { job, onCancel }: Props = $props();

  let progress = $derived(job.progress);
  let percent = $derived(progress?.percent ?? null);
  let status = $derived(progress?.status ?? "downloading");
</script>

<div class="rounded-xl border border-gray-800 bg-gray-900/40 px-4 py-3">
  <div class="flex items-center gap-3">
    <!-- Estado -->
    {#if status === "done"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-emerald-500/10 text-emerald-400">
        <svg class="size-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="m4.5 12.75 6 6 9-13.5" />
        </svg>
      </span>
    {:else if status === "error"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-red-500/10 text-red-400">
        <svg class="size-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
        </svg>
      </span>
    {:else if status === "cancelled"}
      <span class="grid size-6 shrink-0 place-items-center rounded-full bg-gray-800 text-gray-400">
        <svg class="size-3" fill="currentColor" viewBox="0 0 24 24">
          <rect x="6" y="6" width="12" height="12" rx="2" />
        </svg>
      </span>
    {:else}
      <span class="grid size-6 shrink-0 place-items-center text-violet-400">
        <svg class="size-4 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
          <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8v3a5 5 0 0 0-5 5H4z" />
        </svg>
      </span>
    {/if}

    <!-- Título y meta -->
    <div class="min-w-0 flex-1">
      <p class="truncate text-sm text-gray-200">{job.title}</p>
      {#if job.subtitle}
        <p class="mt-0.5 truncate text-[11px] text-gray-600">{job.subtitle}</p>
      {/if}
      {#if status === "downloading"}
        <p class="mt-0.5 text-xs text-gray-500">
          {#if percent != null}
            <span class="tabular-nums text-gray-400">{percent.toFixed(1)}%</span>
            ·
          {/if}
          {formatBytes(progress?.downloadedBytes)}{#if progress?.totalBytes}
            de {formatBytes(progress.totalBytes)}{/if}
          · {formatSpeed(progress?.speedBps)} · ETA {formatEta(progress?.etaSecs)}
        </p>
      {:else if status === "done" && progress?.filePath}
        <p class="mt-0.5 truncate font-mono text-xs text-gray-600">
          {progress.filePath}
        </p>
      {:else if status === "error"}
        <p class="mt-0.5 text-xs text-red-400/90">{progress?.message ?? "Error desconocido"}</p>
      {:else if status === "cancelled"}
        <p class="mt-0.5 text-xs text-gray-500">Cancelado</p>
      {/if}
    </div>

    <!-- Barra de progreso / acción -->
    <div class="flex w-40 shrink-0 items-center justify-end gap-2">
      {#if status === "downloading"}
        <div class="h-1.5 w-full overflow-hidden rounded-full bg-gray-800">
          <div
            class="h-full rounded-full bg-violet-500 transition-[width] duration-300"
            style="width: {Math.min(percent ?? 0, 100)}%"
          ></div>
        </div>
        <button
          class="shrink-0 rounded-md p-1 text-gray-500 transition-colors hover:bg-gray-800 hover:text-gray-300"
          title="Cancelar"
          onclick={() => onCancel(job.id)}
        >
          <svg class="size-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
          </svg>
        </button>
      {/if}
    </div>
  </div>
</div>
