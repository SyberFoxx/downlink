<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { initTheme, theme } from "$lib/theme.svelte";
  import AppearanceView from "$lib/views/AppearanceView.svelte";
  import DownloadsView from "$lib/views/DownloadsView.svelte";
  import HistoryView from "$lib/views/HistoryView.svelte";
  import SettingsView from "$lib/views/SettingsView.svelte";

  let view = $state("descargas");

  onMount(() => {
    void initTheme();
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

<div class="flex h-screen overflow-hidden">
  <Sidebar {view} onNavigate={(v) => (view = v)} />
  <main class="flex-1 overflow-y-auto">
    {#if view === "ajustes"}
      <SettingsView />
    {:else if view === "apariencia"}
      <AppearanceView />
    {:else if view === "historial"}
      <HistoryView onGoToDownloads={() => (view = "descargas")} />
    {:else}
      <DownloadsView onGoToSettings={() => (view = "ajustes")} />
    {/if}
  </main>
</div>
