<script lang="ts">
  import Sidebar from "$lib/components/Sidebar.svelte";
  import DownloadsView from "$lib/views/DownloadsView.svelte";
  import SettingsView from "$lib/views/SettingsView.svelte";

  let view = $state("descargas");
</script>

<svelte:head>
  <title>Downlink</title>
</svelte:head>

<div class="flex h-screen overflow-hidden">
  <Sidebar {view} onNavigate={(v) => (view = v)} />
  <main class="flex-1 overflow-y-auto">
    {#if view === "ajustes"}
      <SettingsView />
    {:else}
      <DownloadsView onGoToSettings={() => (view = "ajustes")} />
    {/if}
  </main>
</div>
