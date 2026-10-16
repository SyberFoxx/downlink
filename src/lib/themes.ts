/**
 * Catálogo de temas y utilidades para aplicarlos.
 *
 * Los temas no son CSS compilado: son mapas de tokens que se escriben como
 * variables CSS en `<html>`. Por eso añadir un tema es añadir un objeto aquí,
 * y el usuario puede ajustar el acento sin tocar nada más.
 */

export type Appearance = "dark" | "light";

export interface ThemeTokens {
  /** Fondo de la ventana. */
  canvas: string;
  /** Superficie de paneles y tarjetas. */
  surface: string;
  /** Superficie elevada (badges, pistas de progreso). */
  elevated: string;
  /** Bordes finos. */
  line: string;
  /** Bordes/hover con más contraste. */
  lineStrong: string;
  /** Texto principal. */
  ink: string;
  /** Texto destacado pero algo más suave. */
  inkStrong: string;
  /** Texto secundario legible. */
  inkSoft: string;
  /** Texto atenuado. */
  inkMuted: string;
  /** Texto apenas visible (placeholders, notas). */
  inkFaint: string;
  /** Color de acento (botones, foco, progreso). */
  accent: string;
  /** Acento en hover. */
  accentHover: string;
  /** Acento suave para texto. */
  accentSoft: string;
  /** Color del texto sobre el acento. */
  accentInk: string;
  /** Errores. */
  danger: string;
  /** Fondo/borde de avisos de error. */
  dangerBg: string;
  /** Éxito. */
  ok: string;
  /** Advertencias. */
  warn: string;
}

export interface Theme {
  id: string;
  label: string;
  appearance: Appearance;
  tokens: ThemeTokens;
}

/** Nombre de la variable CSS de cada token. */
export const TOKEN_VARS: Record<keyof ThemeTokens, string> = {
  canvas: "--canvas",
  surface: "--surface",
  elevated: "--elevated",
  line: "--line",
  lineStrong: "--line-strong",
  ink: "--ink",
  inkStrong: "--ink-strong",
  inkSoft: "--ink-soft",
  inkMuted: "--ink-muted",
  inkFaint: "--ink-faint",
  accent: "--accent",
  accentHover: "--accent-hover",
  accentSoft: "--accent-soft",
  accentInk: "--accent-ink",
  danger: "--danger",
  dangerBg: "--danger-bg",
  ok: "--ok",
  warn: "--warn",
};

/** Mezcla dos colores en oklab; acepta cualquier color CSS válido. */
export function mix(a: string, b: string, pctOfA: number): string {
  return `color-mix(in oklab, ${a} ${pctOfA}%, ${b})`;
}

/** Luminancia relativa aproximada de un color hex (#rgb o #rrggbb). */
function luminance(hex: string): number {
  const clean = hex.replace("#", "");
  const full =
    clean.length === 3
      ? clean
          .split("")
          .map((c) => c + c)
          .join("")
      : clean;
  const num = Number.parseInt(full.slice(0, 6), 16);
  if (Number.isNaN(num)) return 0.5;
  const r = ((num >> 16) & 0xff) / 255;
  const g = ((num >> 8) & 0xff) / 255;
  const b = (num & 0xff) / 255;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Texto legible sobre un color de acento. */
export function inkOn(color: string): string {
  return luminance(color) > 0.6 ? "#111827" : "#ffffff";
}

/** Deriva los tokens de acento para un acento personalizado. */
export function accentOverlay(
  accent: string,
  appearance: Appearance,
): Partial<ThemeTokens> {
  return {
    accent,
    accentHover:
      appearance === "dark"
        ? mix(accent, "#ffffff", 82)
        : mix(accent, "#000000", 88),
    accentSoft: mix(accent, "#ffffff", 60),
    accentInk: inkOn(accent),
  };
}

export const THEMES: Theme[] = [
  {
    id: "downlink-dark",
    label: "Oscuro",
    appearance: "dark",
    tokens: {
      canvas: "#030712",
      surface: "#111827",
      elevated: "#1f2937",
      line: "#1f2937",
      lineStrong: "#374151",
      ink: "#f3f4f6",
      inkStrong: "#d1d5db",
      inkSoft: "#9ca3af",
      inkMuted: "#6b7280",
      inkFaint: "#4b5563",
      accent: "#7c3aed",
      accentHover: "#8b5cf6",
      accentSoft: "#c4b5fd",
      accentInk: "#ffffff",
      danger: "#f87171",
      dangerBg: "#7f1d1d",
      ok: "#34d399",
      warn: "#fbbf24",
    },
  },
  {
    id: "downlink-light",
    label: "Claro",
    appearance: "light",
    tokens: {
      canvas: "#eef0f3",
      surface: "#ffffff",
      elevated: "#e4e7eb",
      line: "#dfe3e8",
      lineStrong: "#cbd2d9",
      ink: "#18181b",
      inkStrong: "#3f3f46",
      inkSoft: "#52525b",
      inkMuted: "#71717a",
      inkFaint: "#a1a1aa",
      accent: "#7c3aed",
      accentHover: "#6d28d9",
      accentSoft: "#7c3aed",
      accentInk: "#ffffff",
      danger: "#dc2626",
      dangerBg: "#fee2e2",
      ok: "#059669",
      warn: "#d97706",
    },
  },
  {
    id: "catppuccin-mocha",
    label: "Catppuccin Mocha",
    appearance: "dark",
    tokens: {
      canvas: "#11111b",
      surface: "#1e1e2e",
      elevated: "#313244",
      line: "#313244",
      lineStrong: "#45475a",
      ink: "#cdd6f4",
      inkStrong: "#bac2de",
      inkSoft: "#a6adc8",
      inkMuted: "#7f849c",
      inkFaint: "#6c7086",
      accent: "#cba6f7",
      accentHover: "#b4befe",
      accentSoft: "#f5c2e7",
      accentInk: "#11111b",
      danger: "#f38ba8",
      dangerBg: mix("#f38ba8", "#11111b", 22),
      ok: "#a6e3a1",
      warn: "#f9e2af",
    },
  },
  {
    id: "catppuccin-latte",
    label: "Catppuccin Latte",
    appearance: "light",
    tokens: {
      canvas: "#e6e9ef",
      surface: "#eff1f5",
      elevated: "#ccd0da",
      line: "#ccd0da",
      lineStrong: "#bcc0cc",
      ink: "#4c4f69",
      inkStrong: "#5c5f77",
      inkSoft: "#6c6f85",
      inkMuted: "#8c8fa1",
      inkFaint: "#9ca0b0",
      accent: "#8839ef",
      accentHover: "#7287fd",
      accentSoft: "#7287fd",
      accentInk: "#ffffff",
      danger: "#d20f39",
      dangerBg: mix("#d20f39", "#e6e9ef", 14),
      ok: "#40a02b",
      warn: "#df8e1d",
    },
  },
  {
    id: "nord",
    label: "Nord",
    appearance: "dark",
    tokens: {
      canvas: "#2e3440",
      surface: "#3b4252",
      elevated: "#434c5e",
      line: "#434c5e",
      lineStrong: "#4c566a",
      ink: "#eceff4",
      inkStrong: "#d8dee9",
      inkSoft: "#aeb8c8",
      inkMuted: "#8690a5",
      inkFaint: "#5a6479",
      accent: "#88c0d0",
      accentHover: "#8fbcbb",
      accentSoft: "#81a1c1",
      accentInk: "#2e3440",
      danger: "#bf616a",
      dangerBg: mix("#bf616a", "#2e3440", 24),
      ok: "#a3be8c",
      warn: "#ebcb8b",
    },
  },
  {
    id: "dracula",
    label: "Dracula",
    appearance: "dark",
    tokens: {
      canvas: "#1e1f29",
      surface: "#282a36",
      elevated: "#44475a",
      line: "#3a3c4e",
      lineStrong: "#44475a",
      ink: "#f8f8f2",
      inkStrong: "#e2e2dc",
      inkSoft: "#c8c9d6",
      inkMuted: "#7b82a8",
      inkFaint: "#6272a4",
      accent: "#bd93f9",
      accentHover: "#cba6ff",
      accentSoft: "#ff79c6",
      accentInk: "#1e1f29",
      danger: "#ff5555",
      dangerBg: mix("#ff5555", "#1e1f29", 22),
      ok: "#50fa7b",
      warn: "#f1fa8c",
    },
  },
  {
    id: "gruvbox-dark",
    label: "Gruvbox",
    appearance: "dark",
    tokens: {
      canvas: "#1d2021",
      surface: "#282828",
      elevated: "#3c3836",
      line: "#3c3836",
      lineStrong: "#504945",
      ink: "#ebdbb2",
      inkStrong: "#d5c4a1",
      inkSoft: "#bdae93",
      inkMuted: "#928374",
      inkFaint: "#665c54",
      accent: "#d65d0e",
      accentHover: "#fe8019",
      accentSoft: "#fabd2f",
      accentInk: "#ffffff",
      danger: "#fb4934",
      dangerBg: mix("#fb4934", "#1d2021", 20),
      ok: "#b8bb26",
      warn: "#fabd2f",
    },
  },
  {
    id: "tokyo-night",
    label: "Tokyo Night",
    appearance: "dark",
    tokens: {
      canvas: "#16161e",
      surface: "#1a1b26",
      elevated: "#292e42",
      line: "#292e42",
      lineStrong: "#414868",
      ink: "#c0caf5",
      inkStrong: "#b4bce8",
      inkSoft: "#a9b1d6",
      inkMuted: "#7d86ad",
      inkFaint: "#565f89",
      accent: "#7aa2f7",
      accentHover: "#89b4ff",
      accentSoft: "#7dcfff",
      accentInk: "#16161e",
      danger: "#f7768e",
      dangerBg: mix("#f7768e", "#16161e", 20),
      ok: "#9ece6a",
      warn: "#e0af68",
    },
  },
  {
    id: "rose-pine",
    label: "Rosé Pine",
    appearance: "dark",
    tokens: {
      canvas: "#191724",
      surface: "#1f1d2e",
      elevated: "#26233a",
      line: "#26233a",
      lineStrong: "#403d52",
      ink: "#e0def4",
      inkStrong: "#ccc9e0",
      inkSoft: "#908caa",
      inkMuted: "#6e6a86",
      inkFaint: "#524f67",
      accent: "#c4a7e7",
      accentHover: "#d0b8f0",
      accentSoft: "#9ccfd8",
      accentInk: "#191724",
      danger: "#eb6f92",
      dangerBg: mix("#eb6f92", "#191724", 20),
      ok: "#31748f",
      warn: "#f6c177",
    },
  },
  {
    id: "solarized-light",
    label: "Solarized Light",
    appearance: "light",
    tokens: {
      canvas: "#eee8d5",
      surface: "#fdf6e3",
      elevated: "#e5deca",
      line: "#ddd6c1",
      lineStrong: "#c9c2ab",
      ink: "#586e75",
      inkStrong: "#657b83",
      inkSoft: "#839496",
      inkMuted: "#93a1a1",
      inkFaint: "#a8b3b0",
      accent: "#268bd2",
      accentHover: "#1f76b4",
      accentSoft: "#2aa198",
      accentInk: "#ffffff",
      danger: "#dc322f",
      dangerBg: mix("#dc322f", "#eee8d5", 14),
      ok: "#859900",
      warn: "#b58900",
    },
  },
];

export const DEFAULT_THEME_ID = "downlink-dark";

/** `true` si el tema sigue el modo claro/oscuro del sistema. */
export function isSystemTheme(id: string): boolean {
  return id === "system";
}

export function themeById(id: string): Theme | undefined {
  return THEMES.find((t) => t.id === id);
}

/** Tema claro/oscuro por defecto para el modo automático. */
export function autoThemeId(preferDark: boolean): string {
  return preferDark ? "downlink-dark" : "downlink-light";
}

/** Paleta cruda exportada por pywal/wallust. */
export interface SystemPalette {
  source: string;
  background: string;
  foreground: string;
  colors: string[];
}

/**
 * Deriva un tema a partir de la paleta del escritorio (pywal/wallust), para que
 * la app combine con el resto del *rice*.
 */
export function themeFromPalette(palette: SystemPalette): Theme {
  const bg = palette.background;
  const fg = palette.foreground;
  const c = (i: number, fallback: string) => palette.colors[i] ?? fallback;
  const light = luminance(bg) > 0.5;
  const accent = c(5, c(4, "#7c3aed"));

  const surface = light ? mix(bg, "#ffffff", 55) : mix(fg, bg, 8);
  const elevated = light ? mix(bg, "#ffffff", 30) : mix(fg, bg, 15);

  return {
    id: "system",
    label: `Escritorio (${palette.source})`,
    appearance: light ? "light" : "dark",
    tokens: {
      canvas: bg,
      surface,
      elevated,
      line: light ? mix(fg, bg, 16) : mix(fg, bg, 18),
      lineStrong: light ? mix(fg, bg, 28) : mix(fg, bg, 28),
      ink: fg,
      inkStrong: mix(fg, bg, 82),
      inkSoft: mix(fg, bg, 66),
      inkMuted: mix(fg, bg, 48),
      inkFaint: mix(fg, bg, 34),
      accent,
      accentHover: light ? mix(accent, "#000000", 85) : mix(accent, "#ffffff", 82),
      accentSoft: mix(accent, fg, 55),
      accentInk: inkOn(accent),
      danger: c(1, "#f87171"),
      dangerBg: mix(c(1, "#f87171"), bg, 22),
      ok: c(2, "#34d399"),
      warn: c(3, "#fbbf24"),
    },
  };
}

/** Escribe los tokens del tema en `<html>`. */
export function applyTokens(tokens: ThemeTokens, appearance: Appearance) {
  const root = document.documentElement;
  for (const [key, cssVar] of Object.entries(TOKEN_VARS)) {
    root.style.setProperty(cssVar, tokens[key as keyof ThemeTokens]);
  }
  root.style.colorScheme = appearance;
  root.dataset.theme = appearance;
}

/** Aplica el velo y el desenfoque de la imagen de fondo. */
export function applyWallpaper(opts: { opacity: number; blur: number }) {
  const root = document.documentElement;
  root.style.setProperty("--wallpaper-blur", `${Math.max(0, opts.blur)}px`);
  root.style.setProperty(
    "--wallpaper-dim",
    String(Math.min(1, Math.max(0, 1 - opts.opacity))),
  );
}
