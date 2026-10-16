import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  cancelDownload,
  clearFinished,
  getQueue,
  onQueueUpdated,
  pauseDownload,
  probeUrl,
  reorderQueue,
  resumeDownload,
  retryDownload,
  startDownload,
} from "./api";
import { defaultOptions } from "./options";
import { extractError } from "./utils";
import type { DownloadOptions, ProbeInfo, QueueJob } from "./types";

/**
 * Tipo MIME interno que marca los arrastres de reordenación de la cola.
 * Sirve para que el arrastre global de enlaces no reaccione a ellos.
 */
export const QUEUE_DRAG_TYPE = "application/x-downlink-queue";

/**
 * Estado de la vista Descargas. Vive en un módulo (y no en el componente) para
 * que el enlace analizado, la selección de la playlist y las opciones elegidas
 * sobrevivan a los cambios de vista: ir a Ajustes y volver no lo pierde todo.
 */
export const downloads = $state({
  url: "",
  probing: false,
  errorMessage: null as string | null,
  probe: null as ProbeInfo | null,
  /** Cola de trabajos; se mantiene al suscribirse desde la raíz de la app. */
  jobs: [] as QueueJob[],
  /** `false` hasta que llega la primera foto de la cola: evita el parpadeo
   *  del estado vacío mientras el backend responde. */
  queueLoaded: false,
  starting: false,
  options: defaultOptions() as DownloadOptions,
  /** Enlaces que se están añadiendo a la cola (contador visible). */
  adding: 0,
  selectedIndices: new Set<number>(),
});

/** Suscribe la cola al evento del backend. Se llama una vez desde la raíz. */
export function startQueueListener(): Promise<() => void> {
  const unlisten = onQueueUpdated((list) => {
    downloads.jobs = list;
    downloads.queueLoaded = true;
  });
  // Refresco inicial: si falla (p. ej. `npm run dev` sin Tauri) la cola queda
  // como está; el error real de cada acción lo verá el usuario donde ocurra.
  getQueue()
    .then((list) => {
      downloads.jobs = list;
    })
    .catch(() => {
      // Sin backend no hay cola que mostrar.
    })
    .finally(() => {
      downloads.queueLoaded = true;
    });
  return unlisten;
}

/** Extrae las URLs únicas de un texto (pegado, arrastre…). */
export function extractUrls(text: string): string[] {
  const matches = text.match(/https?:\/\/[^\s"'<>]+/gi) ?? [];
  return [...new Set(matches.map((m) => m.trim()))];
}

/** Analiza un enlace y deja su vista previa en el panel. */
export async function analyze(raw?: string): Promise<void> {
  const target = (raw ?? downloads.url).trim();
  if (!target || downloads.probing) return;
  downloads.url = target;
  downloads.probing = true;
  downloads.errorMessage = null;
  downloads.probe = null;
  try {
    const info = await probeUrl(target);
    downloads.probe = info;
    downloads.selectedIndices =
      info.kind === "playlist"
        ? new Set(info.entries.map((_, index) => index))
        : new Set();
  } catch (e) {
    downloads.errorMessage = extractError(e);
  } finally {
    downloads.probing = false;
  }
}

/** Un enlace → previsualiza; varios → van directos a la cola. */
export async function handleIncoming(text: string): Promise<void> {
  const urls = extractUrls(text);
  if (urls.length === 0) return;
  if (urls.length === 1) {
    await analyze(urls[0]);
    return;
  }
  await enqueueUrls(urls);
}

async function enqueueUrls(urls: string[]): Promise<void> {
  const snapshot = $state.snapshot(downloads.options) as DownloadOptions;
  downloads.adding = urls.length;
  for (const link of urls) {
    try {
      const info = await probeUrl(link);
      if (info.kind === "playlist") {
        for (const entry of info.entries) {
          await startDownload(entry.url, entry.title, snapshot, entry.thumbnail);
        }
      } else {
        await startDownload(info.webpageUrl, info.title, snapshot, info.thumbnail);
      }
    } catch (e) {
      downloads.errorMessage = extractError(e);
    } finally {
      downloads.adding = Math.max(0, downloads.adding - 1);
    }
  }
  downloads.adding = 0;
}

/** Encola el vídeo o la selección de la playlist analizada. */
export async function download(): Promise<void> {
  const probe = downloads.probe;
  if (!probe || downloads.starting) return;

  const targets =
    probe.kind === "playlist"
      ? probe.entries
          .filter((_, index) => downloads.selectedIndices.has(index))
          .map((entry) => ({
            url: entry.url,
            title: entry.title,
            thumbnail: entry.thumbnail,
          }))
      : [
          {
            url: probe.webpageUrl,
            title: probe.title,
            thumbnail: probe.thumbnail,
          },
        ];

  if (targets.length === 0) return;

  downloads.starting = true;
  downloads.errorMessage = null;
  const snapshot = $state.snapshot(downloads.options) as DownloadOptions;
  try {
    for (const target of targets) {
      await startDownload(target.url, target.title, snapshot, target.thumbnail);
    }
    downloads.probe = null;
    downloads.url = "";
    downloads.selectedIndices = new Set();
  } catch (e) {
    downloads.errorMessage = extractError(e);
  } finally {
    downloads.starting = false;
  }
}

// Acciones sobre un trabajo de la cola. Los errores se quedan en el panel de
// la vista de Descargas, junto a quien los provocó.

export async function pause(id: number): Promise<void> {
  try {
    await pauseDownload(id);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function resume(id: number): Promise<void> {
  try {
    await resumeDownload(id);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function cancel(id: number): Promise<void> {
  try {
    await cancelDownload(id);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function retry(id: number): Promise<void> {
  try {
    await retryDownload(id);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function reorder(ids: number[]): Promise<void> {
  try {
    await reorderQueue(ids);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function reveal(job: QueueJob): Promise<void> {
  if (!job.filePath) return;
  try {
    await revealItemInDir(job.filePath);
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}

export async function clear(): Promise<void> {
  try {
    await clearFinished();
  } catch (e) {
    downloads.errorMessage = extractError(e);
  }
}
