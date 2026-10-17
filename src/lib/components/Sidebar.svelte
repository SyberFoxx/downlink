<script lang="ts">
  import Logo from "$lib/components/Logo.svelte";
  import type { AppView } from "$lib/types";

  interface Props {
    view: AppView;
    onNavigate: (view: AppView) => void;
  }

  let { view, onNavigate }: Props = $props();

  // Cada elemento lleva el/los trazados de su icono (Heroicons 24 outline),
  // dibujados con el mismo grosor para que la barra se vea coherente.
  const items: { id: AppView; label: string; icon: string[] }[] = [
    {
      id: "descargas",
      label: "Descargas",
      icon: [
        "M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3",
      ],
    },
    {
      id: "historial",
      label: "Historial",
      icon: ["M12 6v6h4.5m4.5 0a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"],
    },
    {
      id: "apariencia",
      label: "Apariencia",
      icon: [
        "M4.098 19.902a3.75 3.75 0 0 0 5.304 0l6.401-6.402M6.75 21A3.75 3.75 0 0 1 3 17.25V4.125C3 3.504 3.504 3 4.125 3h5.25c.621 0 1.125.504 1.125 1.125v4.072M6.75 21a3.75 3.75 0 0 0 3.75-3.75V8.197M6.75 21h13.125c.621 0 1.125-.504 1.125-1.125v-5.25c0-.621-.504-1.125-1.125-1.125h-4.072M10.5 8.197l2.88-2.88c.438-.439 1.15-.439 1.59 0l3.712 3.713c.44.44.44 1.152 0 1.59l-2.879 2.88M6.75 17.25h.008v.008H6.75v-.008Z",
      ],
    },
    {
      id: "ajustes",
      label: "Ajustes",
      icon: [
        "M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z",
        "M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z",
      ],
    },
  ];
</script>

<aside
  class="flex w-56 shrink-0 flex-col border-r border-line/70 max-md:order-last max-md:w-full max-md:flex-row max-md:border-t max-md:border-r-0 max-md:border-line/70 max-md:bg-surface/70 max-md:pb-[env(safe-area-inset-bottom)]"
>
  <div
    class="flex h-14 items-center gap-2.5 border-b border-line/70 px-4 max-md:hidden"
  >
    <Logo size={26} />
    <span class="text-[15px] font-semibold tracking-tight">Downlink</span>
  </div>

  <nav
    class="mt-3 space-y-0.5 px-2 max-md:mt-0 max-md:flex max-md:flex-1 max-md:gap-1 max-md:space-y-0 max-md:px-1.5 max-md:py-1.5"
  >
    {#each items as item (item.id)}
      <button
        class="flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-left text-sm transition-colors max-md:min-h-11 max-md:w-auto max-md:flex-1 max-md:flex-col max-md:justify-center max-md:gap-1 max-md:rounded-xl max-md:px-1 max-md:py-1 max-md:text-xs {view ===
        item.id
          ? 'bg-surface text-ink max-md:bg-accent/10 max-md:text-accent-soft'
          : 'text-ink-soft md:hover:bg-surface/50 md:hover:text-ink max-md:active:bg-surface/50'}"
        aria-current={view === item.id ? "page" : undefined}
        onclick={() => onNavigate(item.id)}
      >
        <svg
          class="size-4 shrink-0 max-md:size-5 {view === item.id
            ? 'text-accent-soft max-md:text-accent'
            : 'text-ink-muted'}"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="1.75"
          aria-hidden="true"
        >
          {#each item.icon as d}
            <path stroke-linecap="round" stroke-linejoin="round" {d} />
          {/each}
        </svg>
        {item.label}
      </button>
    {/each}
  </nav>

  <div
    class="mt-auto px-4 py-3 text-[11px] text-ink-faint max-md:hidden"
  >
    v0.10.0-dev · potenciado por yt-dlp
  </div>
</aside>
