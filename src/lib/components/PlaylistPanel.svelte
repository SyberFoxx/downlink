<script lang="ts">
  import type { PlaylistEntry } from "$lib/types";
  import { formatDuration } from "$lib/utils";

  interface Props {
    entries: PlaylistEntry[];
    selected: Set<string>;
    onToggle: (id: string) => void;
    onAll: () => void;
    onNone: () => void;
  }

  let { entries, selected, onToggle, onAll, onNone }: Props = $props();

  let count = $derived(entries.length);
  let allSelected = $derived(selected.size === count);
</script>

<section class="mt-3 overflow-hidden rounded-xl border border-gray-800 bg-gray-900/30">
  <div
    class="flex items-center justify-between gap-3 border-b border-gray-800/70 px-4 py-2.5"
  >
    <div class="flex items-baseline gap-2">
      <h2 class="text-sm font-medium text-gray-300">Vídeos de la playlist</h2>
      <span class="text-xs text-gray-600">{selected.size} de {count} seleccionados</span>
    </div>
    <button
      class="shrink-0 rounded-md border border-gray-800 px-2.5 py-1 text-[11px] text-gray-400 transition-colors hover:border-gray-700 hover:text-gray-200"
      onclick={allSelected ? onNone : onAll}
    >
      {allSelected ? "Quitar todos" : "Seleccionar todos"}
    </button>
  </div>

  <div class="max-h-80 overflow-y-auto">
    {#each entries as entry, index (entry.id || index)}
      <label
        class="flex cursor-pointer items-center gap-3 border-b border-gray-800/40 px-4 py-2 transition-colors last:border-b-0 hover:bg-gray-800/20"
      >
        <input
          type="checkbox"
          class="size-3.5 shrink-0 accent-violet-600"
          checked={selected.has(entry.id)}
          onchange={() => onToggle(entry.id)}
        />
        <span class="w-6 shrink-0 text-right text-[11px] tabular-nums text-gray-600">
          {index + 1}
        </span>
        <span class="min-w-0 flex-1 truncate text-sm text-gray-300">{entry.title}</span>
        {#if entry.duration}
          <span class="shrink-0 text-[11px] tabular-nums text-gray-600">
            {formatDuration(entry.duration)}
          </span>
        {/if}
      </label>
    {/each}
  </div>
</section>
