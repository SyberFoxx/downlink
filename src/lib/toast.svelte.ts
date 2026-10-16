export type ToastKind = "error" | "success" | "info";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

let items = $state<ToastItem[]>([]);
let nextId = 0;

/** Cuánto se queda cada tipo de aviso en pantalla. */
const DURATION: Record<ToastKind, number> = {
  error: 7000,
  success: 3500,
  info: 4000,
};

function dismiss(id: number): void {
  items = items.filter((item) => item.id !== id);
}

function show(kind: ToastKind, message: string): void {
  const id = ++nextId;
  items = [...items, { id, kind, message }];
  setTimeout(() => dismiss(id), DURATION[kind]);
}

/**
 * Avisos globales que se muestran en la esquina inferior derecha (`Toasts.svelte`).
 * Sirven para errores que antes pasaban en silencio, como un guardado fallido.
 */
export const toast = {
  get items(): ToastItem[] {
    return items;
  },
  error: (message: string) => show("error", message),
  success: (message: string) => show("success", message),
  info: (message: string) => show("info", message),
  dismiss,
};
