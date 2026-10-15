export type MediaKind = "video" | "playlist";

export interface PlaylistEntry {
  id: string;
  title: string;
  url: string;
  duration: number | null;
  thumbnail: string | null;
}

export interface ProbeInfo {
  kind: MediaKind;
  title: string;
  channel: string | null;
  duration: number | null;
  thumbnail: string | null;
  webpageUrl: string;
  extractor: string | null;
  formatsCount: number;
  heights: number[];
  entries: PlaylistEntry[];
  /** `true` si la lista se recortó (p. ej. una radio de YouTube sin fin). */
  truncated: boolean;
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
  thumbnail: string | null;
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

/** Una descarga completada, guardada en el historial. */
export interface HistoryEntry {
  url: string;
  title: string;
  thumbnail: string | null;
  filePath: string | null;
  options: DownloadOptions;
  sizeBytes: number | null;
  /** Milisegundos desde el epoch. */
  finishedAt: number;
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
