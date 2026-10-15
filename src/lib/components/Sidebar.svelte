<script lang="ts">
  interface Props {
    view: string;
    onNavigate: (view: string) => void;
  }

  let { view, onNavigate }: Props = $props();

  const items = [
    { id: "descargas", label: "Descargas" },
    { id: "historial", label: "Historial", soon: true },
    { id: "ajustes", label: "Ajustes" },
  ];
</script>

<aside class="flex w-56 shrink-0 flex-col border-r border-gray-800/70">
  <div class="flex h-14 items-center gap-2.5 border-b border-gray-800/70 px-4">
    <div
      class="grid size-6 place-items-center rounded-md bg-violet-600 text-xs font-bold text-white shadow-sm shadow-violet-600/40"
    >
      ↓
    </div>
    <span class="text-[15px] font-semibold tracking-tight">Downlink</span>
  </div>

  <nav class="mt-3 space-y-0.5 px-2">
    {#each items as item (item.id)}
      <button
        class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors {view ===
        item.id
          ? 'bg-gray-900 text-gray-100'
          : 'text-gray-400 hover:bg-gray-900/50 hover:text-gray-200'} {item.soon
          ? 'cursor-not-allowed'
          : ''}"
        disabled={item.soon}
        onclick={() => onNavigate(item.id)}
      >
        {item.label}
        {#if item.soon}
          <span class="ml-auto text-[10px] tracking-wide text-gray-600"
            >próximamente</span
          >
        {/if}
      </button>
    {/each}
  </nav>

  <div class="mt-auto px-4 py-3 text-[11px] text-gray-600">
    v0.4.1-dev · potenciado por yt-dlp
  </div>
</aside>
