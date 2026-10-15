import type { DownloadOptions, MediaMode } from "./types";

export function defaultOptions(): DownloadOptions {
  return {
    mode: "video",
    videoHeight: 1080,
    container: "mp4",
    audioFormat: "mp3",
    audioQuality: "320K",
    volumeDb: 0,
    normalizeAudio: false,
    embedMetadata: true,
    embedThumbnail: true,
  };
}

export const VIDEO_CONTAINERS = [
  { id: "mp4", label: "MP4 — máxima compatibilidad" },
  { id: "mkv", label: "MKV — mejor calidad de códecs" },
  { id: "webm", label: "WEBM — VP9/AV1" },
];

export const AUDIO_FORMATS = [
  { id: "mp3", label: "MP3" },
  { id: "m4a", label: "M4A (AAC)" },
  { id: "vorbis", label: "OGG (Vorbis)" },
  { id: "opus", label: "OPUS" },
  { id: "flac", label: "FLAC — sin pérdida" },
  { id: "wav", label: "WAV — sin comprimir" },
  { id: "best", label: "Original — sin recomprimir" },
];

export const AUDIO_QUALITIES = [
  { id: "320K", label: "320 kbps" },
  { id: "256K", label: "256 kbps" },
  { id: "192K", label: "192 kbps" },
  { id: "128K", label: "128 kbps" },
];

/** Formatos sin recompresión posible (no aplica calidad kbps). */
const LOSSLESS_OR_ORIGINAL = new Set(["flac", "wav", "best"]);

export function qualityApplies(options: DownloadOptions): boolean {
  return !LOSSLESS_OR_ORIGINAL.has(options.audioFormat ?? "best");
}

export type PresetId =
  | "video-1080"
  | "best-quality"
  | "mp3-320"
  | "flac"
  | "audio-original";

export interface Preset {
  id: PresetId;
  label: string;
  values: Partial<DownloadOptions>;
}

export const PRESETS: Preset[] = [
  {
    id: "best-quality",
    label: "Máxima calidad",
    values: {
      mode: "video",
      videoHeight: null,
      container: "mkv",
      embedMetadata: true,
      embedThumbnail: true,
    },
  },
  {
    id: "mp3-320",
    label: "MP3 320k",
    values: {
      mode: "audio",
      audioFormat: "mp3",
      audioQuality: "320K",
      embedMetadata: true,
      embedThumbnail: true,
    },
  },
  {
    id: "flac",
    label: "FLAC sin pérdida",
    values: {
      mode: "audio",
      audioFormat: "flac",
      embedMetadata: true,
      embedThumbnail: true,
    },
  },
  {
    id: "audio-original",
    label: "Audio original",
    values: {
      mode: "audio",
      audioFormat: "best",
      volumeDb: 0,
      normalizeAudio: false,
      embedMetadata: false,
      embedThumbnail: false,
    },
  },
];

/** Preset de 1080p ajustado a la resolución más cercana disponible. */
export function videoPreset(height: number | null): Preset {
  return {
    id: "video-1080",
    label: height ? `Video ${height}p` : "Video (mejor disponible)",
    values: {
      mode: "video",
      videoHeight: height,
      container: "mp4",
      embedMetadata: true,
      embedThumbnail: true,
    },
  };
}

/** Resolución más alta disponible que no supere `limit`. */
export function closestHeight(heights: number[], limit: number): number | null {
  const candidates = heights.filter((h) => h <= limit);
  if (candidates.length > 0) return Math.max(...candidates);
  return heights.length > 0 ? Math.min(...heights) : null;
}

/** Etiqueta corta para mostrar en la fila de descarga. */
export function summarizeOptions(options: DownloadOptions): string {
  const parts: string[] = [];

  if (options.mode === "audio") {
    const format =
      AUDIO_FORMATS.find((f) => f.id === (options.audioFormat ?? "best"))?.label ??
      "Audio";
    parts.push(
      qualityApplies(options) && options.audioQuality
        ? `${format} ${options.audioQuality.replace("K", " kbps")}`
        : format,
    );
  } else {
    const container = (options.container ?? "auto").toUpperCase();
    parts.push(
      `${options.videoHeight ? `${options.videoHeight}p` : "Mejor"} · ${container}`,
    );
  }

  if (options.volumeDb) {
    parts.push(`${options.volumeDb > 0 ? "+" : ""}${options.volumeDb} dB`);
  }
  if (options.normalizeAudio) parts.push("normalizado");
  if (options.embedMetadata || options.embedThumbnail) parts.push("con metadatos");

  return parts.join(" · ");
}

export function modeLabel(mode: MediaMode): string {
  return mode === "audio" ? "Audio" : "Video";
}
