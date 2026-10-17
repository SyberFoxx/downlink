import { engineUpdateState, onEngineUpdate } from "./api";
import type { EngineUpdatePhase, EngineUpdateProgress } from "./types";

/**
 * Progreso de la descarga o actualización de los motores gestionados:
 * yt-dlp, ffmpeg y el runtime JS (quickjs).
 *
 * Vive en un módulo y no en `EngineUpdateRow` porque la app se descarga los
 * motores ella sola al arrancar: el aviso tiene que verse desde cualquier
 * vista, no solo desde Ajustes.
 */
export const engines = $state({
  /** Último progreso recibido; `null` si esta sesión no ha habido ninguno. */
  progress: null as EngineUpdateProgress | null,
});

/** Fases en las que hay algo en marcha; el resto es finalizar o fallar. */
const ACTIVE_PHASES: EngineUpdatePhase[] = [
  "checking",
  "downloading",
  "verifying",
  "installing",
];

/** Nombres mostrables para cada id de motor. */
const ENGINE_NAMES: Record<string, string> = {
  "yt-dlp": "yt-dlp",
  ffmpeg: "ffmpeg",
  quickjs: "runtime JS",
};

export function isActivePhase(phase: EngineUpdatePhase): boolean {
  return ACTIVE_PHASES.includes(phase);
}

/** Nombre del motor para los textos ("quickjs" no debe aparecer en la UI). */
export function engineName(engine: string): string {
  return ENGINE_NAMES[engine] ?? engine;
}

/**
 * Suscribe el aviso global de motor a los eventos del backend. Se llama una
 * sola vez, desde la raíz.
 *
 * Además de escuchar, recupera el último progreso registrado: `auto_update` se
 * lanza en el arranque y los primeros eventos se emiten mientras la UI todavía
 * se está montando, con lo que un evento que nadie escucha se perdería.
 */
export function startEngineListener(): Promise<() => void> {
  const unlisten = onEngineUpdate((progress) => {
    engines.progress = progress;
  });
  engineUpdateState()
    .then((progress) => {
      // Solo si no ha llegado nada mientras se hidrataba: los eventos son más
      // recientes que el estado guardado en memoria.
      if (progress && engines.progress === null) engines.progress = progress;
    })
    .catch(() => {
      // Sin backend no hay nada que mostrar.
    });
  return unlisten;
}
