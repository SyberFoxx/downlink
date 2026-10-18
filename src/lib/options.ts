import type { DownloadOptions, MediaMode } from "./types";

/** Resoluciones de reserva cuando el enlace no las reporta (p. ej. playlists). */
export const DEFAULT_HEIGHTS = [2160, 1440, 1080, 720, 480, 360];

/** Resoluciones a ofrecer: las reales del enlace o un set estándar si no hay. */
export function availableHeightsFor(heights: number[]): number[] {
  return heights.length > 0 ? heights : DEFAULT_HEIGHTS;
}

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
    clipStart: null,
    clipEnd: null,
    subtitles: false,
    subtitleLangs: "es",
    subtitleAuto: true,
    embedSubtitles: true,
    sponsorblock: false,
    sponsorblockMode: "remove",
    sponsorblockCategories: ["sponsor", "selfpromo"],
  };
}

/** Segundos → "mm:ss" o "h:mm:ss" (para mostrar en la UI). */
export function formatTimecode(seconds?: number | null): string {
  if (seconds == null || !isFinite(seconds) || seconds < 0) return "";
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return (h > 0 ? `${h}:` : "") + `${mm}:${String(s).padStart(2, "0")}`;
}

/**
 * Interpreta "90", "1:30" o "1:02:03" como segundos. Devuelve `null` si el
 * texto está vacío o no es un tiempo válido.
 */
export function parseTimecode(value: string): number | null {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parts = trimmed.split(":");
  if (parts.length > 3) return null;
  const numbers: number[] = [];
  for (const part of parts) {
    if (!/^\d+(\.\d+)?$/.test(part.trim())) return null;
    numbers.push(Number(part));
  }
  return numbers.reduce((acc, value) => acc * 60 + value, 0);
}

export const VIDEO_CONTAINERS = [
  { id: "mp4", label: "MP4 — máxima compatibilidad (hasta 1080p)" },
  { id: "mkv", label: "MKV — mejor calidad de códecs" },
  { id: "webm", label: "WEBM — VP9/AV1" },
];

/**
 * Highest resolution MP4 keeps without losing quality. In MP4 the backend asks
 * for H.264 + AAC, and H.264 stops at 1080p, so 1440p+ would come out as 1080p.
 * Mirrors `MAX_MP4_HEIGHT` in `src-tauri/src/ytdlp.rs`.
 */
export const MAX_MP4_HEIGHT = 1080;

/** Container actually used: above the MP4 limit (or "best") MKV is used. */
export function effectiveContainer(options: DownloadOptions): string | null {
  const tooBigForMp4 =
    options.videoHeight == null || options.videoHeight > MAX_MP4_HEIGHT;
  if (options.mode === "video" && options.container === "mp4" && tooBigForMp4) {
    return "mkv";
  }
  return options.container;
}

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

/**
 * Categorías de SponsorBlock que la app ofrece. Deben coincidir con
 * `SPONSORBLOCK_CATEGORIES` del backend (src-tauri/src/ytdlp.rs).
 */
export const SPONSORBLOCK_CATEGORIES = [
  { id: "sponsor", label: "Patrocinio", description: "Mención pagada del vídeo" },
  { id: "selfpromo", label: "Autopromoción", description: "Promo de sus propios productos" },
  { id: "interaction", label: "Interacción", description: "Petición de likes y suscripción" },
  { id: "intro", label: "Intro", description: "Cabecera o introducción" },
  { id: "outro", label: "Outro", description: "Cierre o despedida" },
  { id: "preview", label: "Avance", description: "Resumen de lo que viene" },
  { id: "filler", label: "Relleno", description: "Divagaciones sin relación" },
  {
    id: "music_offtopic",
    label: "Música fuera de tema",
    description: "Sección musical no musical",
  },
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
    const container = (effectiveContainer(options) ?? "auto").toUpperCase();
    parts.push(
      `${options.videoHeight ? `${options.videoHeight}p` : "Mejor"} · ${container}`,
    );
  }

  if (options.volumeDb) {
    parts.push(`${options.volumeDb > 0 ? "+" : ""}${options.volumeDb} dB`);
  }
  if (options.normalizeAudio) parts.push("normalizado");
  if (options.embedMetadata || options.embedThumbnail) parts.push("con metadatos");

  if (options.clipStart != null) {
    const end =
      options.clipEnd != null ? formatTimecode(options.clipEnd) : "final";
    parts.push(`recorte ${formatTimecode(options.clipStart)}–${end}`);
  }
  if (options.subtitles) {
    parts.push(options.embedSubtitles ? "subs incrustados" : "subtítulos");
  }
  if (options.sponsorblock) {
    parts.push(
      options.sponsorblockMode === "mark" ? "segmentos marcados" : "sin patrocinios",
    );
  }

  return parts.join(" · ");
}

export function modeLabel(mode: MediaMode): string {
  return mode === "audio" ? "Audio" : "Video";
}
