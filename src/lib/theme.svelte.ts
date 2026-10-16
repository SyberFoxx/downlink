import { readSystemPalette, readWallpaper } from "./api";
import { settings } from "./settings.svelte";
import {
  accentOverlay,
  applyTokens,
  applyWallpaper,
  autoThemeId,
  DEFAULT_THEME_ID,
  themeById,
  themeFromPalette,
  type SystemPalette,
  type Theme,
} from "./themes";

let systemDark = $state(true);
let palette = $state<SystemPalette | null>(null);
let wallpaperUrl = $state<string | null>(null);
let started = false;

/** Tema que corresponde aplicar ahora mismo (resuelve "system"). */
export function activeTheme(): Theme {
  const t = settings.value.theme;
  if (t.themeId === "system") {
    if (palette) return themeFromPalette(palette);
    return themeById(autoThemeId(systemDark)) ?? themeById(DEFAULT_THEME_ID)!;
  }
  return themeById(t.themeId) ?? themeById(DEFAULT_THEME_ID)!;
}

function apply() {
  const t = settings.value.theme;
  const base = activeTheme();
  const overlay = t.accent ? accentOverlay(t.accent, base.appearance) : null;
  const tokens = overlay ? { ...base.tokens, ...overlay } : base.tokens;
  applyTokens(tokens, base.appearance);
  applyWallpaper({
    opacity: t.wallpaperOpacity,
    blur: t.wallpaperBlur,
  });
}

/** Lee la imagen de fondo guardada y la deja lista para pintar. */
async function loadWallpaper() {
  const path = settings.value.theme.wallpaper;
  if (!path) {
    wallpaperUrl = null;
    return;
  }
  try {
    wallpaperUrl = await readWallpaper(path);
  } catch {
    wallpaperUrl = null;
  }
}

/**
 * Arranca el sistema de temas: carga ajustes, sigue el modo claro/oscuro del
 * sistema y reaplica el tema cuando cambia cualquier pieza.
 */
export async function initTheme() {
  if (started) return;
  started = true;

  if (typeof window !== "undefined" && window.matchMedia) {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    systemDark = mq.matches;
    mq.addEventListener("change", (event) => (systemDark = event.matches));
  }

  await settings.load();
  await loadWallpaper();
  await refreshPalette();

  $effect.root(() => {
    $effect(apply);
  });
}

/** Vuelve a buscar la paleta de pywal/wallust. */
export async function refreshPalette() {
  try {
    palette = await readSystemPalette();
  } catch {
    palette = null;
  }
}

export const theme = {
  get systemDark(): boolean {
    return systemDark;
  },
  get palette(): SystemPalette | null {
    return palette;
  },
  get wallpaperUrl(): string | null {
    return wallpaperUrl;
  },
  get active(): Theme {
    return activeTheme();
  },
  get followsSystem(): boolean {
    return settings.value.theme.themeId === "system";
  },

  async setTheme(id: string) {
    settings.value.theme.themeId = id;
    await settings.save();
  },

  async setAccent(hex: string | null) {
    settings.value.theme.accent = hex;
    await settings.save();
  },

  async setWallpaper(path: string | null) {
    if (!path) return this.clearWallpaper();
    const url = await readWallpaper(path);
    settings.value.theme.wallpaper = path;
    wallpaperUrl = url;
    await settings.save();
  },

  async clearWallpaper() {
    settings.value.theme.wallpaper = null;
    wallpaperUrl = null;
    await settings.save();
  },

  /** Ajusta opacidad/desenfoque del fondo y persiste. */
  async setWallpaperLook(opts: { opacity?: number; blur?: number }) {
    if (opts.opacity !== undefined)
      settings.value.theme.wallpaperOpacity = opts.opacity;
    if (opts.blur !== undefined) settings.value.theme.wallpaperBlur = opts.blur;
    await settings.save();
  },

  /** Adopta la paleta del escritorio (pywal/wallust) como tema. */
  async adoptPalette() {
    await refreshPalette();
    if (!palette) return false;
    settings.value.theme.themeId = "system";
    await settings.save();
    return true;
  },

  /** Vuelve todo a los valores por defecto de apariencia. */
  async reset() {
    settings.value.theme.themeId = DEFAULT_THEME_ID;
    settings.value.theme.accent = null;
    settings.value.theme.wallpaper = null;
    settings.value.theme.wallpaperOpacity = 0.35;
    settings.value.theme.wallpaperBlur = 12;
    wallpaperUrl = null;
    await settings.save();
  },
};
