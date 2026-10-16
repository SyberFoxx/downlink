<script lang="ts">
  interface Props {
    view: string;
    onNavigate: (view: string) => void;
  }

  let { view, onNavigate }: Props = $props();

  const items = [
    { id: "descargas", label: "Descargas" },
    { id: "historial", label: "Historial" },
    { id: "apariencia", label: "Apariencia" },
    { id: "ajustes", label: "Ajustes" },
  ];
</script>

<aside class="flex w-56 shrink-0 flex-col border-r border-line/70">
  <div class="flex h-14 items-center gap-2.5 border-b border-line/70 px-4">
    <div
      class="grid size-6 place-items-center rounded-md bg-accent text-xs font-bold text-accent-ink shadow-sm shadow-accent/40"
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
          ? 'bg-surface text-ink'
          : 'text-ink-soft hover:bg-surface/50 hover:text-ink'}"
        onclick={() => onNavigate(item.id)}
      >
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="mt-auto px-4 py-3 text-[11px] text-ink-faint">
    v0.8.0-dev · potenciado por yt-dlp
  </div>
</aside>
