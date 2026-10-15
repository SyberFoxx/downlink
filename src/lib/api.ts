import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { DownloadProgress, ProbeInfo } from "./types";

export function probeUrl(url: string): Promise<ProbeInfo> {
  return invoke<ProbeInfo>("probe_url", { url });
}

export function startDownload(url: string): Promise<number> {
  return invoke<number>("start_download", { url });
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
