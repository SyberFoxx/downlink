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
