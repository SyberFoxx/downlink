<script lang="ts">
  import { engineName, engines, isActivePhase } from "$lib/engine.svelte";

  const PHASE_LABELS: Record<string, string> = {
    checking: "Buscando actualización…",
    downloading: "Descargando…",
    verifying: "Verificando integridad…",
    installing: "Instalando…",
    done: "Actualizado ✓",
    error: "No se pudo actualizar",
  };

  let progress = $derived(engines.progress);
  let active = $derived(progress !== null && isActivePhase(progress.phase));
  let percent = $derived(
    typeof progress?.percent === "number" ? progress.percent : null,
  );

  let visible = $state(false);

  // Al terminar (o fallar) el aviso se queda unos instantes y se va solo.
  $effect(() => {
    if (!progress) {
      visible = false;
      return;
    }
    visible = true;
    if (isActivePhase(progress.phase)) return;
    const ms = progress.phase === "error" ? 6000 : 3000;
    const timer = setTimeout(() => (visible = false), ms);
    return () => clearTimeout(timer);
  });
</script>

<!--
  Barra de motor: avisa de las descargas y actualizaciones de yt-dlp, ffmpeg y
  el runtime JS aunque la app las haga sola al arrancar. Al ser `sticky`, se
  queda fijada arriba del área de contenido en todas las vistas.
-->
{#if visible && progress}
  <div
    class="sticky top-0 z-30 border-b border-line bg-surface/90 backdrop-blur max-md:top-[env(safe-area-inset-top)]"
    role="status"
    aria-live="polite"
  >
    <div class="flex items-center gap-2.5 px-4 py-2 md:px-6">
      {#if active}
        <span class="relative flex size-2 shrink-0">
          <span
            class="absolute size-full animate-ping rounded-full bg-accent opacity-60"
          ></span>
          <span class="relative size-2 rounded-full bg-accent"></span>
        </span>
      {:else}
        <span
          class="size-2 shrink-0 rounded-full {progress.phase === 'error'
            ? 'bg-danger'
            : 'bg-accent'}"
        ></span>
      {/if}

      <span class="shrink-0 text-xs font-medium text-ink-strong">
        {engineName(progress.engine)}
      </span>
      <span
        class="min-w-0 truncate text-xs {progress.phase === 'error'
          ? 'text-danger'
          : 'text-ink-muted'}"
      >
        {progress.phase === "error" && progress.message
          ? progress.message
          : PHASE_LABELS[progress.phase]}
      </span>

      {#if percent !== null}
        <span class="ml-auto shrink-0 text-xs tabular-nums text-ink-muted">
          {Math.round(percent)}%
        </span>
      {/if}
    </div>

    <div class="h-0.5 w-full bg-elevated">
      {#if percent !== null}
        <div
          class="h-full bg-accent transition-[width] duration-200"
          style="width: {percent}%"
        ></div>
      {:else if active}
        <div class="h-full w-1/3 animate-pulse bg-accent/60"></div>
      {/if}
    </div>
  </div>
{/if}
