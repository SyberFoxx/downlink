export interface ProbeInfo {
  title: string;
  channel: string | null;
  duration: number | null;
  thumbnail: string | null;
  webpageUrl: string;
  extractor: string | null;
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
