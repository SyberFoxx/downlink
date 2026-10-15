<script lang="ts">
  import { onMount } from "svelte";
  import {
    cancelDownload,
    onDownloadProgress,
    probeUrl,
    startDownload,
  } from "$lib/api";
  import type { DownloadProgress, ProbeInfo } from "$lib/types";
  import PreviewCard from "$lib/components/PreviewCard.svelte";
  import DownloadRow from "$lib/components/DownloadRow.svelte";

  interface Job {
    id: number;
    title: string;
    progress: DownloadProgress | null;
  }

  interface Props {
    onGoToSettings: () => void;
  }

  let { onGoToSettings }: Props = $props();

  let url = $state("");
  let probing = $state(false);
  let errorMessage = $state<string | null>(null);
  let probe = $state<ProbeInfo | null>(null);
  let jobs = $state<Job[]>([]);
  let starting = $state(false);

  let needsCookies = $derived(
    !!errorMessage &&
      /cookies|anti-bot|verificaci|sign in/i.test(errorMessage),
  );

  function extractError(e: unknown): string {
    if (typeof e === "string") return e;
    if (e instanceof Error) return e.message;
    return "Error inesperado";
  }

  async function analyze(raw?: string) {
    const target = (raw ?? url).trim();
    if (!target || probing) return;
    url = target;
    probing = true;
    errorMessage = null;
    probe = null;
    try {
      probe = await probeUrl(target);
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      probing = false;
    }
  }

  async function download() {
    if (!probe || starting) return;
    starting = true;
    errorMessage = null;
    try {
      const jobId = await startDownload(probe.webpageUrl);
      jobs = [{ id: jobId, title: probe.title, progress: null }, ...jobs];
      probe = null;
      url = "";
    } catch (e) {
      errorMessage = extractError(e);
    } finally {
      starting = false;
    }
  }

  async function cancel(jobId: number) {
    try {
      await cancelDownload(jobId);
    } catch (e) {
      errorMessage = extractError(e);
    }
  }

  onMount(() => {
    const unlisten = onDownloadProgress((progress) => {
      jobs = jobs.map((j) => (j.id === progress.jobId ? { ...j, progress } : j));
    });

    const onPaste = (e: ClipboardEvent) => {
      const text = e.clipboardData?.getData("text")?.trim();
      if (text && /^https?:\/\//i.test(text)) {
        analyze(text);
      }
    };
    window.addEventListener("paste", onPaste);

    return () => {
      unlisten.then((un) => un());
      window.removeEventListener("paste", onPaste);
    };
  });
</script>

<div class="mx-auto max-w-3xl px-10 py-12">
  <header class="mb-8">
    <h1 class="text-lg font-semibold tracking-tight text-gray-100">Descargar</h1>
    <p class="mt-1 text-sm text-gray-500">
      Pega un enlace de YouTube, TikTok, Instagram y +1800 sitios soportados
    </p>
  </header>

  <!-- Input de URL -->
  <div class="relative">
    <svg
      class="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-gray-500"
      fill="none"
      viewBox="0 0 24 24"
      stroke="currentColor"
      stroke-width="2"
    >
      <path stroke-linecap="round" stroke-linejoin="round" d="M13.19 8.688a4.5 4.5 0 0 1 1.242 7.244l-4.5 4.5a4.5 4.5 0 0 1-6.364-6.364l1.757-1.757m13.35-.687 1.757-1.757a4.5 4.5 0 0 0-6.364-6.364l-4.5 4.5a4.5 4.5 0 0 0 1.242 7.244" />
    </svg>
    <input
      type="url"
      bind:value={url}
      placeholder="https://www.youtube.com/watch?v=..."
      class="w-full rounded-lg border border-gray-800 bg-gray-900/50 py-3 pl-10 pr-28 text-sm text-gray-100 outline-none transition-colors placeholder:text-gray-600 focus:border-violet-600"
      onkeydown={(e) => {
        if (e.key === "Enter") analyze();
      }}
    />
    <button
      class="absolute right-1.5 top-1.5 bottom-1.5 rounded-md bg-violet-600 px-4 text-sm font-medium text-white shadow-sm shadow-violet-600/30 transition-colors hover:bg-violet-500 disabled:cursor-not-allowed disabled:opacity-50"
      disabled={probing || !url.trim()}
      onclick={() => analyze()}
    >
      {#if probing}
        <span class="flex items-center gap-2">
          <svg class="size-3.5 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" />
            <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8v3a5 5 0 0 0-5 5H4z" />
          </svg>
          Analizando…
        </span>
      {:else}
        Analizar
      {/if}
    </button>
  </div>

  <p class="mt-2 text-xs text-gray-600">
    Tip: pega el enlace directamente con <span class="font-mono text-gray-500">Ctrl+V</span> en
    cualquier parte de la ventana
  </p>

  <!-- Error -->
  {#if errorMessage}
    <div
      class="mt-4 rounded-lg border border-red-900/50 bg-red-950/30 px-4 py-3 text-sm text-red-400"
    >
      <p class="whitespace-pre-line">{errorMessage}</p>
      {#if needsCookies}
        <button
          class="mt-2.5 rounded-md bg-red-900/40 px-3 py-1.5 text-xs font-medium text-red-200 transition-colors hover:bg-red-900/60"
          onclick={onGoToSettings}
        >
          Configurar cookies en Ajustes →
        </button>
      {/if}
    </div>
  {/if}

  <!-- Preview del enlace analizado -->
  {#if probe}
    <div class="mt-6">
      <PreviewCard info={probe} onDownload={download} />
    </div>
  {/if}

  <!-- Descargas -->
  {#if jobs.length > 0}
    <section class="mt-10">
      <div class="mb-3 flex items-baseline justify-between">
        <h2 class="text-sm font-medium text-gray-300">Descargas</h2>
        <span class="text-xs text-gray-600">{jobs.length} en cola</span>
      </div>
      <div class="space-y-2">
        {#each jobs as job (job.id)}
          <DownloadRow {job} onCancel={cancel} />
        {/each}
      </div>
    </section>
  {/if}
</div>
