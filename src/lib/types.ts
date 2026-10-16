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

/** Qué hacer con los segmentos detectados por SponsorBlock. */
export type SponsorBlockMode = "remove" | "mark";

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
  /** Recorte: segundo de inicio del fragmento. */
  clipStart: number | null;
  /** Recorte: segundo de fin (null = hasta el final). */
  clipEnd: number | null;
  /** Descarga subtítulos. */
  subtitles: boolean;
  /** Idiomas de subtítulos separados por coma (p. ej. "es,en"). */
  subtitleLangs: string;
  /** Incluye subtítulos automáticos. */
  subtitleAuto: boolean;
  /** Incrusta los subtítulos en el vídeo en vez de dejar archivos .srt. */
  embedSubtitles: boolean;
  /** Quita o marca segmentos promocionales con SponsorBlock (solo YouTube). */
  sponsorblock: boolean;
  /** Qué hacer con los segmentos: quitar o marcar como capítulos. */
  sponsorblockMode: SponsorBlockMode;
  /** Categorías a tratar (vacío = `sponsor`). */
  sponsorblockCategories: string[];
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

/** Vistas principales de la app (las que muestra la barra lateral). */
export type AppView = "descargas" | "historial" | "apariencia" | "ajustes";

/** Preferencias de apariencia. */
export interface ThemeSettings {
  /** Id de un tema del catálogo, o "system" para seguir al escritorio. */
  themeId: string;
  /** Acento personalizado en hex, o null para el del tema. */
  accent: string | null;
  /** Ruta a la imagen de fondo elegida por el usuario. */
  wallpaper: string | null;
  /** Cuánto se ve la imagen de fondo (0–1). */
  wallpaperOpacity: number;
  /** Desenfoque de la imagen de fondo, en píxeles. */
  wallpaperBlur: number;
}

export interface Settings {
  cookiesMode: CookiesMode;
  cookiesBrowser: string | null;
  cookiesFile: string | null;
  concurrency: number;
  /** Busca y aplica actualizaciones de yt-dlp al arrancar. */
  autoUpdateYtdlp: boolean;
  /** Descarga ffmpeg al arrancar si no hay ninguno disponible. */
  autoDownloadFfmpeg: boolean;
  /** Carpeta de destino de las descargas (null = la del sistema). */
  downloadDir: string | null;
  /** Reanuda las descargas pendientes al abrir la app. */
  persistQueue: boolean;
  /** Muestra una notificación del sistema al terminar una descarga. */
  notifyOnFinish: boolean;
  /** Mantiene la app en la bandeja al cerrar la ventana. */
  closeToTray: boolean;
  /** Apariencia: tema, acento e imagen de fondo. */
  theme: ThemeSettings;
}

/** Paleta exportada por pywal/wallust. */
export interface SystemPalette {
  source: string;
  background: string;
  foreground: string;
  colors: string[];
}

export interface BrowserInfo {
  id: string;
  label: string;
}

export interface EngineInfo {
  ytdlpVersion: string | null;
  ytdlpSource: string | null;
  ytdlpPath: string | null;
  jsRuntime: string | null;
  ffmpegPath: string | null;
  ffmpegVersion: string | null;
  ffmpegSource: string | null;
}

/** Resultado de comprobar si hay una versión nueva de un motor. */
export interface UpdateCheck {
  current: string | null;
  latest: string | null;
  updateAvailable: boolean;
}

export type EngineUpdatePhase =
  | "checking"
  | "downloading"
  | "verifying"
  | "installing"
  | "done"
  | "error";

/** Progreso de la actualización del motor (evento `engine-update`). */
export interface EngineUpdateProgress {
  engine: string;
  phase: EngineUpdatePhase;
  percent: number | null;
  message: string | null;
}
