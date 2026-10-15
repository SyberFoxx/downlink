import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  BrowserInfo,
  DownloadOptions,
  DownloadProgress,
  EngineInfo,
  ProbeInfo,
  Settings,
} from "./types";

export function probeUrl(url: string): Promise<ProbeInfo> {
  return invoke<ProbeInfo>("probe_url", { url });
}

export function startDownload(
  url: string,
  options: DownloadOptions,
): Promise<number> {
  return invoke<number>("start_download", { url, options });
}

export function cancelDownload(jobId: number): Promise<void> {
  return invoke<void>("cancel_download", { id: jobId });
}

export function onDownloadProgress(
  callback: (progress: DownloadProgress) => void,
): Promise<() => void> {
  return listen<DownloadProgress>("download-progress", (event) =>
    callback(event.payload),
  );
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

export async function pickCookiesFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: "Selecciona un archivo cookies.txt",
    filters: [{ name: "Cookies", extensions: ["txt"] }],
  });
  return typeof selected === "string" ? selected : null;
}
