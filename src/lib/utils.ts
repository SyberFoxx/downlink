export function formatDuration(seconds?: number | null): string {
  if (seconds == null || !isFinite(seconds) || seconds < 0) return "--:--";
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return (h > 0 ? `${h}:` : "") + `${mm}:${String(s).padStart(2, "0")}`;
}

export function formatBytes(bytes?: number | null): string {
  if (bytes == null || bytes <= 0) return "--";
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i++;
  }
  return `${value.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatSpeed(bytesPerSecond?: number | null): string {
  if (bytesPerSecond == null || bytesPerSecond <= 0) return "--";
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatEta(seconds?: number | null): string {
  if (seconds == null || seconds < 0) return "--:--";
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

/** Referencia accesible para un error; lo normal es que venga como string. */
export function extractError(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Error inesperado";
}

/**
 * ¿Corre dentro del WebView de Android? Solo ahí existen el *share intent* y el
 * aviso de enlaces compartidos; en el escritorio no hay nada que recibir.
 */
export function isAndroid(): boolean {
  return typeof navigator !== "undefined" && /Android/i.test(navigator.userAgent);
}

/**
 * Enseña al usuario el archivo descargado.
 *
 * En el escritorio se abre el gestor de archivos con el fichero seleccionado
 * (`revealItemInDir`). En Android esa API **no existe** —devolvía «API not
 * supported on the current platform»— y tampoco tiene sentido: no hay gestor de
 * archivos que «seleccione» nada. Ahí lo que el usuario espera es **abrir** el
 * vídeo con la app que corresponda.
 *
 * En Android el archivo puede ser un `content://` de la galería, que el plugin
 * `opener` rechaza (valida con `metadata()`), así que se usa el comando propio
 * `open_file`, que lanza el `ACTION_VIEW` del sistema.
 */
export async function revealOrOpen(
  path: string,
  revealItemInDir: (path: string) => Promise<void>,
  openFile: (path: string) => Promise<void>,
): Promise<void> {
  if (isAndroid()) {
    await openFile(path);
    return;
  }
  await revealItemInDir(path);
}

/**
 * Texto que se enseña como «ubicación» del archivo descargado.
 *
 * En Android, tras publicar el archivo en la galería, la ruta es un
 * `content://media/external/video/media/1234`: correcto para el sistema, pero
 * ilegible para una persona. Se sustituye por una frase que dice dónde está de
 * verdad. En el escritorio la ruta real sí es útil, así que se deja tal cual.
 */
export function displayPath(path: string): string {
  if (!path.startsWith("content://")) return path;
  return "Guardado en la galería del dispositivo";
}

const DATE_FORMAT = new Intl.DateTimeFormat("es", {
  day: "2-digit",
  month: "short",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

/** Fecha legible a partir de milisegundos desde el epoch. */
export function formatDate(millis: number): string {
  if (!Number.isFinite(millis) || millis <= 0) return "--";
  return DATE_FORMAT.format(new Date(millis));
}
