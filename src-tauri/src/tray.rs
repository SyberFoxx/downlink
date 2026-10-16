//! Icono de bandeja: acceso rápido a la ventana, estado de la cola y salida.

use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::queue::{JobStatus, JobView};

/// Texto puesto en el tooltip la última vez, para no reescribirlo en cada tick.
static LAST_TOOLTIP: Mutex<String> = Mutex::new(String::new());

const TRAY_ID: &str = "downlink-tray";
const MENU_SHOW: &str = "show";
const MENU_QUIT: &str = "quit";

/// Acorta un texto largo para el tooltip, sin partir por la mitad de un carácter.
fn shorten(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// Texto del tooltip según el estado de la cola (función pura, testeable).
fn tooltip_text(jobs: &[JobView]) -> String {
    let active: Vec<&JobView> = jobs.iter().filter(|j| j.status.is_active()).collect();
    let queued = jobs
        .iter()
        .filter(|j| j.status == JobStatus::Queued)
        .count();

    match (active.as_slice(), queued) {
        ([], 0) => "Downlink".to_string(),
        ([job], 0) => format!(
            "Downlink — {} · {:.0} %",
            shorten(&job.title, 30),
            job.percent.unwrap_or(0.0)
        ),
        ([job], queued) => format!(
            "Downlink — {} · {:.0} % (+{} en cola)",
            shorten(&job.title, 30),
            job.percent.unwrap_or(0.0),
            queued
        ),
        (active, 0) => format!("Downlink — {} descargas activas", active.len()),
        (active, queued) => format!(
            "Downlink — {} descargas activas (+{} en cola)",
            active.len(),
            queued
        ),
    }
}

/// Crea el icono de bandeja con su menú. Devuelve error si el sistema no lo
/// soporta; en ese caso la app sigue funcionando con normalidad.
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, MENU_SHOW, "Mostrar Downlink", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Salir", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        // En Linux/AppIndicator el clic izquierdo abre el menú; el atajo de
        // mostrar la ventana vive en el propio menú.
        .show_menu_on_left_click(false)
        .tooltip("Downlink")
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_SHOW => show_window(app),
            MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Refresca el tooltip con el estado de la cola. Solo escribe cuando el texto
/// cambia, así que se puede llamar en cada actualización de progreso.
pub fn sync_tooltip(app: &AppHandle, jobs: &[JobView]) {
    let text = tooltip_text(jobs);
    {
        let Ok(mut last) = LAST_TOOLTIP.lock() else {
            return;
        };
        if *last == text {
            return;
        }
        *last = text.clone();
    }
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let _ = tray.set_tooltip(Some(&text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{JobStatus, JobView};
    use crate::ytdlp::DownloadOptions;

    fn job(id: u64, title: &str, status: JobStatus, percent: Option<f64>) -> JobView {
        let mut view = JobView::new(
            id,
            format!("https://x/{id}"),
            title.to_string(),
            None,
            DownloadOptions::default(),
        );
        view.status = status;
        view.percent = percent;
        view
    }

    #[test]
    fn tooltip_sin_descargas() {
        assert_eq!(tooltip_text(&[]), "Downlink");
    }

    #[test]
    fn tooltip_con_una_descarga_muestra_titulo_y_porcentaje() {
        let text = tooltip_text(&[job(1, "Un vídeo", JobStatus::Downloading, Some(42.0))]);
        assert_eq!(text, "Downlink — Un vídeo · 42 %");
    }

    #[test]
    fn tooltip_una_activa_mas_pendientes() {
        let text = tooltip_text(&[
            job(1, "Un vídeo", JobStatus::Downloading, Some(10.0)),
            job(2, "Otro", JobStatus::Queued, None),
        ]);
        assert!(text.contains("+1 en cola"), "texto inesperado: {text}");
    }

    #[test]
    fn tooltip_con_varias_activas_resume_el_total() {
        let text = tooltip_text(&[
            job(1, "A", JobStatus::Downloading, Some(50.0)),
            job(2, "B", JobStatus::Processing, Some(30.0)),
            job(3, "C", JobStatus::Queued, None),
        ]);
        assert!(
            text.contains("2 descargas activas"),
            "texto inesperado: {text}"
        );
        assert!(text.contains("+1 en cola"), "texto inesperado: {text}");
    }

    #[test]
    fn tooltip_ignora_los_terminados() {
        let text = tooltip_text(&[
            job(1, "Hecho", JobStatus::Done, Some(100.0)),
            job(2, "Falló", JobStatus::Error, None),
        ]);
        assert_eq!(text, "Downlink");
    }

    #[test]
    fn tooltip_acorta_titulos_largos() {
        let largo = "Este es un título extremadamente largo que debe acortarse para el tooltip";
        let text = tooltip_text(&[job(1, largo, JobStatus::Downloading, Some(5.0))]);
        assert!(text.contains('…'), "debería acortarse: {text}");
        assert!(text.chars().count() < 70, "demasiado largo: {text}");
    }
}
