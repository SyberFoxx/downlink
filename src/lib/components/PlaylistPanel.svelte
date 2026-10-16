<script lang="ts">
  import type { PlaylistEntry } from "$lib/types";
  import { formatDuration } from "$lib/utils";

  interface Props {
    entries: PlaylistEntry[];
    /** Índices seleccionados (no ids: las radios pueden repetir vídeos). */
    selected: Set<number>;
    truncated?: boolean;
    onToggle: (index: number) => void;
    onAll: () => void;
    onNone: () => void;
  }

  let { entries, selected, truncated = false, onToggle, onAll, onNone }: Props =
    $props();

  let count = $derived(entries.length);
  let allSelected = $derived(selected.size === count);
</script>

<section class="mt-3 overflow-hidden rounded-xl border border-line bg-surface/30">
  <div
    class="flex items-center justify-between gap-3 border-b border-line/70 px-4 py-2.5"
  >
    <div class="flex items-baseline gap-2">
      <h2 class="text-sm font-medium text-ink-strong">Vídeos de la playlist</h2>
      <span class="text-xs text-ink-faint">{selected.size} de {count} seleccionados</span>
    </div>
    <button
      class="shrink-0 rounded-md border border-line px-2.5 py-1 text-[11px] text-ink-soft transition-colors hover:border-line-strong hover:text-ink"
      onclick={allSelected ? onNone : onAll}
    >
      {allSelected ? "Quitar todos" : "Seleccionar todos"}
    </button>
  </div>

  {#if truncated}
    <p class="border-b border-line/40 bg-surface/40 px-4 py-2 text-[11px] text-warn/80">
      Es una radio de YouTube: se listan los primeros {count} vídeos.
    </p>
  {/if}

  <div class="max-h-80 overflow-y-auto">
    {#each entries as entry, index (index)}
      <label
        class="flex cursor-pointer items-center gap-3 border-b border-line/40 px-4 py-2 transition-colors last:border-b-0 hover:bg-elevated/20"
      >
        <input
          type="checkbox"
          class="size-3.5 shrink-0 accent-accent"
          checked={selected.has(index)}
          onchange={() => onToggle(index)}
        />
        <span class="w-6 shrink-0 text-right text-[11px] tabular-nums text-ink-faint">
          {index + 1}
        </span>
        <span class="min-w-0 flex-1 truncate text-sm text-ink-strong">{entry.title}</span>
        {#if entry.duration}
          <span class="shrink-0 text-[11px] tabular-nums text-ink-faint">
            {formatDuration(entry.duration)}
          </span>
        {/if}
      </label>
    {/each}
  </div>
</section>
