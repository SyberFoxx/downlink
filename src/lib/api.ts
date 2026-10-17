import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BrowserInfo,
  DownloadOptions,
  EngineInfo,
  EngineUpdateProgress,
  HistoryEntry,
  ProbeInfo,
  QueueJob,
  Settings,
  SystemPalette,
  UpdateCheck,
} from "./types";

export function probeUrl(url: string): Promise<ProbeInfo> {
  return invoke<ProbeInfo>("probe_url", { url });
}

export function startDownload(
  url: string,
  title: string | null,
  options: DownloadOptions,
  thumbnail: string | null = null,
): Promise<number> {
  return invoke<number>("start_download", { url, title, thumbnail, options });
}

export function pauseDownload(jobId: number): Promise<void> {
  return invoke<void>("pause_download", { id: jobId });
}

export function resumeDownload(jobId: number): Promise<void> {
  return invoke<void>("resume_download", { id: jobId });
}

export function cancelDownload(jobId: number): Promise<void> {
  return invoke<void>("cancel_download", { id: jobId });
}

/** Reintenta un trabajo fallido o cancelado, conservando sus opciones. */
export function retryDownload(jobId: number): Promise<void> {
  return invoke<void>("retry_download", { id: jobId });
}

export function reorderQueue(ids: number[]): Promise<void> {
  return invoke<void>("reorder_queue", { ids });
}

export function clearFinished(): Promise<void> {
  return invoke<void>("clear_finished");
}

export function getQueue(): Promise<QueueJob[]> {
  return invoke<QueueJob[]>("get_queue");
}

export function onQueueUpdated(
  callback: (jobs: QueueJob[]) => void,
): Promise<() => void> {
  return listen<QueueJob[]>("queue-updated", (event) => callback(event.payload));
}

export function getHistory(): Promise<HistoryEntry[]> {
  return invoke<HistoryEntry[]>("get_history");
}

export function removeHistory(url: string): Promise<void> {
  return invoke<void>("remove_history", { url });
}

export function clearHistory(): Promise<void> {
  return invoke<void>("clear_history");
}

export function getSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke<void>("save_settings", { settings });
}

export function detectBrowsers(): Promise<BrowserInfo[]> {
  return invoke<BrowserInfo[]>("detect_browsers");
}

export function engineInfo(): Promise<EngineInfo> {
  return invoke<EngineInfo>("engine_info");
}

export function checkYtdlpUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("check_ytdlp_update");
}

export function updateYtdlp(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("update_ytdlp");
}

export function checkFfmpegUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("check_ffmpeg_update");
}

export function updateFfmpeg(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("update_ffmpeg");
}

export function checkJsRuntimeUpdate(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("check_js_runtime_update");
}

export function updateJsRuntime(): Promise<UpdateCheck> {
  return invoke<UpdateCheck>("update_js_runtime");
}

export function onEngineUpdate(
  callback: (progress: EngineUpdateProgress) => void,
): Promise<() => void> {
  return listen<EngineUpdateProgress>("engine-update", (event) =>
    callback(event.payload),
  );
}

export async function pickCookiesFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: "Selecciona un archivo cookies.txt",
    filters: [{ name: "Cookies", extensions: ["txt"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickWallpaper(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: "Selecciona una imagen de fondo",
    filters: [
      {
        name: "Imágenes",
        extensions: ["png", "jpg", "jpeg", "webp", "gif", "bmp", "avif"],
      },
    ],
  });
  return typeof selected === "string" ? selected : null;
}

/** Elige la carpeta donde guardar las descargas. */
export async function pickDownloadFolder(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Elige la carpeta de descargas",
  });
  return typeof selected === "string" ? selected : null;
}

/** Carpeta de descargas por defecto del sistema. */
export function defaultDownloadDir(): Promise<string | null> {
  return invoke<string | null>("default_download_dir");
}

/** ¿Está disponible la bandeja del sistema? */
export function trayAvailable(): Promise<boolean> {
  return invoke<boolean>("tray_available");
}

/** Lee la imagen de fondo y la devuelve como data URL lista para usar en CSS. */
export function readWallpaper(path: string): Promise<string> {
  return invoke<string>("read_wallpaper", { path });
}

/** Paleta del escritorio generada por pywal/wallust, si existe. */
export function readSystemPalette(): Promise<SystemPalette | null> {
  return invoke<SystemPalette | null>("read_system_palette");
}
