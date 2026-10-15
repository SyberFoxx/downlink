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

export type DownloadStatus = "downloading" | "done" | "error" | "cancelled";

export interface DownloadProgress {
  jobId: number;
  percent: number | null;
  downloadedBytes: number | null;
  totalBytes: number | null;
  speedBps: number | null;
  etaSecs: number | null;
  status: DownloadStatus;
  filePath: string | null;
  message: string | null;
}

export type CookiesMode = "none" | "browser" | "file";

export interface Settings {
  cookiesMode: CookiesMode;
  cookiesBrowser: string | null;
  cookiesFile: string | null;
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
