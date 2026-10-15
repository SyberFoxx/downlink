export interface ProbeInfo {
  title: string;
  channel: string | null;
  duration: number | null;
  thumbnail: string | null;
  webpageUrl: string;
  extractor: string | null;
  formatsCount: number;
  heights: number[];
}

export type JobStatus =
  | "queued"
  | "downloading"
  | "processing"
  | "paused"
  | "done"
  | "error"
  | "cancelled";

export type MediaMode = "video" | "audio";

export interface DownloadOptions {
  mode: MediaMode;
  videoHeight: number | null;
  container: string | null;
  audioFormat: string | null;
  audioQuality: string | null;
  volumeDb: number | null;
  normalizeAudio: boolean;
  embedMetadata: boolean;
  embedThumbnail: boolean;
}

/** Un trabajo de la cola, tal como lo reporta el backend. */
export interface QueueJob {
  id: number;
  url: string;
  title: string;
  options: DownloadOptions;
  status: JobStatus;
  percent: number | null;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speedBps: number | null;
  etaSecs: number | null;
  filePath: string | null;
  message: string | null;
}

export type CookiesMode = "none" | "browser" | "file";

export interface Settings {
  cookiesMode: CookiesMode;
  cookiesBrowser: string | null;
  cookiesFile: string | null;
  concurrency: number;
}

export interface BrowserInfo {
  id: string;
  label: string;
}

export interface EngineInfo {
  ytdlpVersion: string | null;
  jsRuntime: string | null;
  ffmpegPath: string | null;
  ffmpegVersion: string | null;
}
