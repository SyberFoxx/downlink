import { revealItemInDir } from "@tauri-apps/plugin-opener";
import {
  cancelDownload,
  clearFinished,
  getQueue,
  onQueueUpdated,
  onSharedLink,
  openFile,
  pauseDownload,
  probeUrl,
  reorderQueue,
  resumeDownload,
  retryDownload,
  startDownload,
  takeSharedText,
} from "./api";
import { defaultOptions } from "./options";
import { settings } from "./settings.svelte";
import { toast } from "./toast.svelte";
import { extractError, isAndroid, revealOrOpen } from "./utils";
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

/** Aviso cuando el texto recibido no traía ningún enlace que analizar. */
const NO_URL_MESSAGE = "No he encontrado ningún enlace en el texto recibido.";

/** Puntuación que arrastra el texto que rodea a un enlace al compartirlo. */
const TRAILING_PUNCTUATION = /[.,;:!?…]+$/;

/** Pares de cierre que sobran cuando el enlace venía entre paréntesis. */
const BRACKETS: ReadonlyArray<readonly [string, string]> = [
  ["(", ")"],
  ["[", "]"],
  ["{", "}"],
];

function countChar(text: string, char: string): number {
  let total = 0;
  for (const current of text) if (current === char) total++;
  return total;
}

/**
 * Quita del final de la URL lo que en realidad es del texto que la envuelve:
 * «mira esto https://…» acaba en punto, y «(https://…)» en paréntesis. Los
 * paréntesis solo se quitan si no cierran un par abierto dentro de la URL
 * (una URL con «(desambiguación)» los conserva).
 */
function trimUrl(url: string): string {
  let out = url.replace(TRAILING_PUNCTUATION, "");
  for (;;) {
    let changed = false;
    const punctuation = out.replace(TRAILING_PUNCTUATION, "");
    if (punctuation !== out) {
      out = punctuation;
      changed = true;
    }
    for (const [open, close] of BRACKETS) {
      if (out.endsWith(close) && countChar(out, close) > countChar(out, open)) {
        out = out.slice(0, -1);
        changed = true;
      }
    }
    if (!changed) return out;
  }
}

/** Extrae las URLs únicas de un texto (pegado, arrastre, otra app…). */
export function extractUrls(text: string): string[] {
  const matches = text.match(/https?:\/\/[^\s"'<>]+/gi) ?? [];
  const urls = matches.map((match) => trimUrl(match.trim())).filter(Boolean);
  return [...new Set(urls)];
}

/**
 * Analiza un enlace y deja su vista previa en el panel. Acepta también el texto
 * completo que comparte otra app (título y enlace en el mismo bloque).
 */
export async function analyze(raw?: string): Promise<void> {
  const text = (raw ?? downloads.url).trim();
  const target = extractUrls(text)[0] ?? text;
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
  if (urls.length === 0) {
    toast.info(NO_URL_MESSAGE);
    return;
  }
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

/**
 * Enlace recibido desde otra app por el *share intent* de Android. Respeta lo
 * elegido en Ajustes: descargarlo ya (como pegar el enlace en la consola) o
 * dejar la vista previa para elegir opciones. Si vienen varios enlaces a la vez
 * van todos a la cola, igual que al pegarlos.
 */
export async function handleShared(text: string): Promise<void> {
  const urls = extractUrls(text);
  if (urls.length === 0) {
    toast.info(NO_URL_MESSAGE);
    return;
  }
  if (urls.length > 1 || settings.value.shareAction === "preview") {
    await handleIncoming(text);
    return;
  }
  await downloadDirect(urls[0]);
}

/** Analiza el enlace y lo encola sin pedir confirmación. */
async function downloadDirect(link: string): Promise<void> {
  if (downloads.probing) return;
  downloads.url = link;
  downloads.probing = true;
  downloads.errorMessage = null;
  downloads.probe = null;
  const snapshot = $state.snapshot(downloads.options) as DownloadOptions;
  try {
    const info = await probeUrl(link);
    // Una lista de reproducción puede ser enorme: mejor que la revise el usuario.
    if (info.kind === "playlist") {
      downloads.probe = info;
      downloads.selectedIndices = new Set(info.entries.map((_, index) => index));
      return;
    }
    await startDownload(info.webpageUrl, info.title, snapshot, info.thumbnail);
    toast.success(`En la cola: ${info.title}`);
  } catch (e) {
    const message = extractError(e);
    downloads.errorMessage = message;
    // El aviso del panel no se ve si el usuario sigue en otra vista: la app se
    // abrió para descargar y hay que decirle que no ha podido.
    toast.error(message);
  } finally {
    downloads.probing = false;
  }
}

/** Texto compartido ya tratado, para no repetirlo si llega por las dos vías. */
let lastShared: { text: string; at: number } | null = null;

/**
 * Recibe los enlaces que comparten otras apps. Solo tiene sentido en Android:
 * en el escritorio no hay *share intent* y devuelve un no-op.
 *
 * Las dos vías se solapan a propósito. Al compartir, Android arranca la app, así
 * que lo normal es que el enlace espere en el backend (`take_shared_text`) a que
 * exista la interfaz; si ya estaba abierta, llega por el evento `share-intent`.
 * El duplicado se descarta por si llegan las dos.
 */
export async function startShareListener(
  handle: (text: string) => void,
): Promise<() => void> {
  if (!isAndroid()) return () => {};
  // El ajuste decide qué se hace con el enlace: hay que tenerlo antes de recibir
  // nada, o un enlace entrante usaría el valor por defecto sin querer.
  await settings.load().catch(() => undefined);
  const once = (text: string): void => {
    const now = Date.now();
    if (lastShared && lastShared.text === text && now - lastShared.at < 2000) {
      return;
    }
    lastShared = { text, at: now };
    handle(text);
  };
  const unlisten = await onSharedLink(once).catch(() => () => {});
  takeSharedText()
    .then((pending) => {
      if (pending) once(pending);
    })
    .catch(() => undefined);
  return unlisten;
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
    await revealOrOpen(job.filePath, revealItemInDir, openFile);
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
