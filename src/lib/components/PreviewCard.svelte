<script lang="ts">
  import type { ProbeInfo } from "$lib/types";
  import { formatDuration } from "$lib/utils";

  interface Props {
    info: ProbeInfo;
  }

  let { info }: Props = $props();
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
      {#if info.kind === "playlist"}
        <span class="font-medium text-violet-300">
          Playlist · {info.entries.length} elemento{info.entries.length === 1 ? "" : "s"}
        </span>
      {:else}
        <span>{formatDuration(info.duration)}</span>
        {#if info.formatsCount > 0}
          <span class="text-gray-700">·</span>
          <span>{info.formatsCount} formatos</span>
        {/if}
      {/if}
      {#if info.extractor}
        <span class="text-gray-700">·</span>
        <span>{info.extractor}</span>
      {/if}
    </div>

    <div class="mt-auto flex items-center justify-end gap-2 pt-3">
      {#if info.heights.length > 0}
        <span class="rounded-full border border-gray-800 px-2 py-0.5 text-[10px] text-gray-500">
          hasta {info.heights[0]}p
        </span>
      {/if}
    </div>
  </div>
</div>
