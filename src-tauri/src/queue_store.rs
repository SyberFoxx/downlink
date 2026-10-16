use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::queue::JobView;

/// Lo que se guarda en `queue.json`: los trabajos pendientes y el siguiente id,
/// para que al reabrir la app no se repitan identificadores.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedQueue {
    pub next_id: u64,
    pub jobs: Vec<JobView>,
}

/// Ruta del archivo de la cola dentro de la carpeta de configuración.
fn queue_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| "No se pudo resolver la carpeta de configuración.".to_string())?;
    Ok(dir.join("queue.json"))
}

/// Lee la cola guardada. Devuelve `None` si no existe o está corrupta: es un
/// dato de conveniencia, nunca debe impedir que la app arranque.
pub fn load(app: &AppHandle) -> Option<PersistedQueue> {
    let path = queue_path(app).ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

/// Guarda la cola pendiente. Los fallos se devuelven para poder registrarlos.
pub fn save(app: &AppHandle, queue: &PersistedQueue) -> Result<(), String> {
    let path = queue_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear la carpeta de configuración: {e}"))?;
    }
    let json =
        serde_json::to_string(queue).map_err(|e| format!("No se pudo serializar la cola: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("No se pudo guardar la cola: {e}"))
}

/// Borra la cola guardada (al desactivar la persistencia en Ajustes).
pub fn clear(app: &AppHandle) -> Result<(), String> {
    let path = queue_path(app)?;
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("No se pudo borrar la cola guardada: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{JobStatus, JobView};
    use crate::ytdlp::DownloadOptions;

    fn sample() -> PersistedQueue {
        let mut job = JobView::new(
            3,
            "https://example.com/v".to_string(),
            "Un vídeo".to_string(),
            None,
            DownloadOptions::default(),
        );
        job.status = JobStatus::Paused;
        PersistedQueue {
            next_id: 5,
            jobs: vec![job],
        }
    }

    #[test]
    fn la_cola_va_y_vuelve_en_json() {
        let original = sample();
        let json = serde_json::to_string(&original).expect("serializa");
        let restored: PersistedQueue = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(restored.next_id, 5);
        assert_eq!(restored.jobs.len(), 1);
        assert_eq!(restored.jobs[0].id, 3);
        assert_eq!(restored.jobs[0].title, "Un vídeo");
        assert_eq!(restored.jobs[0].status, JobStatus::Paused);
    }

    #[test]
    fn una_cola_corrupta_no_revienta() {
        assert!(serde_json::from_str::<PersistedQueue>("{no soy json").is_err());
    }
}
