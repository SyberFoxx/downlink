import { getSettings, saveSettings } from "./api";
import { toast } from "./toast.svelte";
import { extractError } from "./utils";
import type { Settings } from "./types";

/** Ajustes por defecto; también sirven de reserva si el backend no responde. */
export const DEFAULT_SETTINGS: Settings = {
  cookiesMode: "none",
  cookiesBrowser: null,
  cookiesFile: null,
  concurrency: 2,
  autoUpdateYtdlp: true,
  autoDownloadFfmpeg: true,
  theme: {
    themeId: "downlink-dark",
    accent: null,
    wallpaper: null,
    wallpaperOpacity: 0.35,
    wallpaperBlur: 12,
  },
};

let value = $state<Settings>(structuredClone(DEFAULT_SETTINGS));
let loaded = $state(false);
let inflight: Promise<Settings> | null = null;

/**
 * Fuente de verdad única de los ajustes. La comparten Ajustes y Apariencia
 * para que guardar en una vista no pise lo de la otra.
 */
export const settings = {
  get value(): Settings {
    return value;
  },
  get loaded(): boolean {
    return loaded;
  },
  /** Carga los ajustes una sola vez (comparte la petición en curso). */
  async load(): Promise<Settings> {
    if (loaded) return value;
    inflight ??= (async () => {
      try {
        value = await getSettings();
      } catch {
        // Sin backend (p. ej. `npm run dev` sin Tauri) se usan los valores por defecto.
      }
      loaded = true;
      return value;
    })();
    return inflight;
  },
  /**
   * Persiste el estado actual. Devuelve `false` si falla (y avisa con un
   * toast): así ningún llamante puede quedarse creyendo que guardó.
   */
  async save(): Promise<boolean> {
    try {
      await saveSettings($state.snapshot(value) as Settings);
      return true;
    } catch (e) {
      toast.error(`No se pudieron guardar los ajustes. ${extractError(e)}`);
      return false;
    }
  },
};
