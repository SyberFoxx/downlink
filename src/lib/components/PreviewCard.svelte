<script lang="ts">
  import type { ProbeInfo } from "$lib/types";
  import { formatDuration } from "$lib/utils";

  interface Props {
    info: ProbeInfo;
    onDownload: () => void;
  }

  let { info, onDownload }: Props = $props();
</script>

<div
  class="flex gap-4 rounded-xl border border-gray-800 bg-gray-900/40 p-4"
>
  {#if info.thumbnail}
    <img
      src={info.thumbnail}
      alt=""
      class="aspect-video w-48 shrink-0 rounded-lg bg-gray-900 object-cover"
    />
  {:else}
    <div
      class="grid aspect-video w-48 shrink-0 place-items-center rounded-lg bg-gray-900 text-gray-600"
    >
      <svg class="size-8" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
        <path stroke-linecap="round" stroke-linejoin="round" d="m15.75 10.5 4.72-4.72a.75.75 0 0 1 1.28.53v11.38a.75.75 0 0 1-1.28.53l-4.72-4.72M4.5 18.75h9a2.25 2.25 0 0 0 2.25-2.25v-9a2.25 2.25 0 0 0-2.25-2.25h-9A2.25 2.25 0 0 0 2.25 7.5v9a2.25 2.25 0 0 0 2.25 2.25Z" />
      </svg>
    </div>
  {/if}

  <div class="flex min-w-0 flex-1 flex-col">
    <p class="line-clamp-2 text-sm font-medium leading-snug text-gray-100">
      {info.title}
    </p>

    <div class="mt-1.5 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-gray-500">
      {#if info.channel}
        <span>{info.channel}</span>
        <span class="text-gray-700">·</span>
      {/if}
      <span>{formatDuration(info.duration)}</span>
      {#if info.formatsCount > 0}
        <span class="text-gray-700">·</span>
        <span>{info.formatsCount} formatos</span>
      {/if}
      {#if info.extractor}
        <span class="text-gray-700">·</span>
        <span>{info.extractor}</span>
      {/if}
    </div>

    <div class="mt-auto flex items-center justify-end pt-3">
      <button
        class="flex items-center gap-1.5 rounded-md bg-violet-600 px-3.5 py-1.5 text-sm font-medium text-white shadow-sm shadow-violet-600/30 transition-colors hover:bg-violet-500 active:bg-violet-600"
        onclick={onDownload}
      >
        <svg class="size-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
        </svg>
        Descargar
      </button>
    </div>
  </div>
</div>
