<script lang="ts">
  import { fly } from "svelte/transition";
  import { toast, type ToastKind } from "$lib/toast.svelte";

  const LABEL: Record<ToastKind, string> = {
    error: "Error",
    success: "Hecho",
    info: "Aviso",
  };
</script>

<!-- Pila de avisos globales. `aria-live` anuncia cada aviso sin robar el foco. -->
<div
  class="pointer-events-none fixed bottom-4 right-4 z-[60] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2 max-md:bottom-[calc(var(--nav-height)+env(safe-area-inset-bottom)+0.5rem)]"
  role="status"
  aria-live="polite"
>
  {#each toast.items as item (item.id)}
    <div
      transition:fly={{ y: 8, duration: 160 }}
      class="pointer-events-auto flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 text-sm shadow-lg shadow-black/20 {item.kind ===
      'error'
        ? 'border-danger-bg/60 bg-danger-bg/40 text-danger'
        : item.kind === 'success'
          ? 'border-ok/40 bg-surface text-ok'
          : 'border-line bg-surface text-ink'}"
    >
      <div class="min-w-0 flex-1">
        <p class="text-[10px] font-semibold uppercase tracking-wide opacity-70">
          {LABEL[item.kind]}
        </p>
        <p class="mt-0.5 break-words whitespace-pre-line">{item.message}</p>
      </div>
      <button
        class="tap tap-center -m-0.5 shrink-0 rounded-md p-1 opacity-60 transition-opacity hover:opacity-100"
        aria-label="Cerrar aviso"
        onclick={() => toast.dismiss(item.id)}
      >
        <svg
          class="size-3.5"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          stroke-width="2"
        >
          <path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  {/each}
</div>
