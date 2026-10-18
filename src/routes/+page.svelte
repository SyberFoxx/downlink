<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import EngineIndicator from "$lib/components/EngineIndicator.svelte";
  import Toasts from "$lib/components/Toasts.svelte";
  import { initTheme, theme } from "$lib/theme.svelte";
  import { startEngineListener } from "$lib/engine.svelte";
  import {
    QUEUE_DRAG_TYPE,
    extractUrls,
    handleIncoming,
    handleShared,
    startQueueListener,
    startShareListener,
  } from "$lib/downloads.svelte";
  import type { AppView } from "$lib/types";
  import AppearanceView from "$lib/views/AppearanceView.svelte";
  import DownloadsView from "$lib/views/DownloadsView.svelte";
  import HistoryView from "$lib/views/HistoryView.svelte";
  import SettingsView from "$lib/views/SettingsView.svelte";

  let view = $state<AppView>("descargas");
  let dropActive = $state(false);
  let dragDepth = 0;
  /** Contenedor con scroll: al cambiar de vista volvemos arriba. */
  let mainEl = $state<HTMLElement | null>(null);

  // Si el sistema pide reducir el movimiento, la transición de vista es instantánea.
  const reduceMotion =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const viewTransition = reduceMotion ? { duration: 0 } : { y: 8, duration: 180 };

  $effect(() => {
    // Al cambiar de vista, la nueva empieza desde arriba (evita heredar el
    // scroll de la vista anterior).
    void view;
    if (mainEl) mainEl.scrollTop = 0;
  });

  /**
   * Enlace recibido desde fuera (pegado o arrastre, en cualquier vista):
   * salta a Descargas para que el usuario vea el resultado y lo analiza.
   */
  async function onIncoming(text: string) {
    if (extractUrls(text).length === 0) return;
    view = "descargas";
    await handleIncoming(text);
  }

  /**
   * Enlace compartido desde otra app (Android): salta a Descargas para que se vea
   * el resultado y lo trata según lo elegido en Ajustes.
   */
  async function onShared(text: string) {
    view = "descargas";
    await handleShared(text);
  }

  function isExternalDrag(e: DragEvent): boolean {
    const types = e.dataTransfer?.types;
    if (!types) return false;
    // El reordenamiento interno de la cola no debe abrir el overlay de soltar.
    if (types.includes(QUEUE_DRAG_TYPE)) return false;
    return (
      types.includes("text/uri-list") ||
      types.includes("text/plain") ||
      types.includes("Files")
    );
  }

  function onDragEnter(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    dragDepth += 1;
    dropActive = true;
  }

  function onDragLeave() {
    dragDepth = Math.max(0, dragDepth - 1);
    if (dragDepth === 0) dropActive = false;
  }

  function onDragOver(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  }

  function onDrop(e: DragEvent) {
    if (!isExternalDrag(e)) return;
    e.preventDefault();
    dragDepth = 0;
    dropActive = false;
    const text =
      e.dataTransfer?.getData("text/uri-list") ||
      e.dataTransfer?.getData("text/plain") ||
      "";
    void onIncoming(text);
  }

  onMount(() => {
    void initTheme();
    // La cola se suscribe aquí (raíz) para que el estado se mantenga aunque
    // el usuario esté en Historial o Ajustes.
    const unlistenQueue = startQueueListener();
    // Igual con la barra de motores: la app se descarga yt-dlp/ffmpeg al
    // arrancar y el aviso tiene que verse aunque estés en otra vista.
    const unlistenEngine = startEngineListener();
    // Y con los enlaces que comparten otras apps (solo Android).
    const unlistenShare = startShareListener(onShared);

    const onPaste = (e: ClipboardEvent) => {
      const target = e.target;
      // En un campo de texto se pega con normalidad (el de URL analiza solo).
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }
      const text = e.clipboardData?.getData("text")?.trim();
      if (text && /^https?:\/\//i.test(text)) void onIncoming(text);
    };

    window.addEventListener("paste", onPaste);
    window.addEventListener("dragenter", onDragEnter);
    window.addEventListener("dragleave", onDragLeave);
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);

    return () => {
      unlistenQueue.then((un) => un());
      unlistenEngine.then((un) => un());
      unlistenShare.then((un) => un());
      window.removeEventListener("paste", onPaste);
      window.removeEventListener("dragenter", onDragEnter);
      window.removeEventListener("dragleave", onDragLeave);
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
    };
  });
</script>

<svelte:head>
  <title>Downlink</title>
</svelte:head>

<div
  class="app-wallpaper"
  aria-hidden="true"
  style:background-image={theme.wallpaperUrl
    ? `url("${theme.wallpaperUrl}")`
    : "none"}
></div>

<div class="flex h-dvh overflow-hidden max-md:flex-col">
  <Sidebar {view} onNavigate={(v) => (view = v)} />
  <main
    bind:this={mainEl}
    class="min-h-0 flex-1 overflow-y-auto max-md:pt-[env(safe-area-inset-top)]"
  >
    <!-- Fuera de `{#key view}`: el aviso no debe perderse al cambiar de vista. -->
    <EngineIndicator />
    {#key view}
      <div in:fly={viewTransition}>
        {#if view === "ajustes"}
          <SettingsView />
        {:else if view === "apariencia"}
          <AppearanceView />
        {:else if view === "historial"}
          <HistoryView onGoToDownloads={() => (view = "descargas")} />
        {:else}
          <DownloadsView onGoToSettings={() => (view = "ajustes")} />
        {/if}
      </div>
    {/key}
  </main>
</div>

<!-- Resaltado al arrastrar enlaces sobre la ventana -->
{#if dropActive}
  <div
    class="pointer-events-none fixed inset-4 z-50 rounded-2xl border-2 border-dashed border-accent-hover/70 bg-accent-hover/5"
  >
    <div
      class="absolute inset-x-0 top-6 text-center text-sm font-medium text-accent-soft"
    >
      Suelta para añadir a la cola
    </div>
  </div>
{/if}

<Toasts />
