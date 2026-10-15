use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::ytdlp::DownloadOptions;

/// Cuántas descargas se conservan en el historial.
const MAX_ENTRIES: usize = 500;

/// Una descarga completada, guardada en disco.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    /// URL original: identifica la entrada (una descarga por enlace).
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub file_path: Option<String>,
    pub options: DownloadOptions,
    /// Tamaño del archivo final en bytes, si se pudo leer.
    pub size_bytes: Option<u64>,
    /// Momento de finalización en milisegundos desde el epoch.
    pub finished_at: u64,
}

impl HistoryEntry {
    pub fn new(
        url: String,
        title: String,
        thumbnail: Option<String>,
        file_path: Option<String>,
        options: DownloadOptions,
    ) -> Self {
        let size_bytes = file_path
            .as_deref()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|meta| meta.len());
        Self {
            url,
            title,
            thumbnail,
            file_path,
            options,
            size_bytes,
            finished_at: now_millis(),
        }
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Inserta una entrada al frente, reemplazando la previa del mismo enlace y
/// recortando la lista. Función pura para poder probarla.
fn upsert(entries: &mut Vec<HistoryEntry>, entry: HistoryEntry) {
    entries.retain(|existing| existing.url != entry.url);
    entries.insert(0, entry);
    entries.truncate(MAX_ENTRIES);
}

pub struct HistoryState(pub Mutex<Vec<HistoryEntry>>);

fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|_| "No se pudo resolver la carpeta de configuración.".to_string())?;
    Ok(dir.join("history.json"))
}

pub fn load(app: &AppHandle) -> Vec<HistoryEntry> {
    let Ok(path) = history_path(app) else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(app: &AppHandle, entries: &[HistoryEntry]) -> Result<(), String> {
    let path = history_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear la carpeta de configuración: {e}"))?;
    }
    let json = serde_json::to_string(entries)
        .map_err(|e| format!("No se pudo serializar el historial: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("No se pudo guardar el historial: {e}"))
}

impl HistoryState {
    /// Guarda una descarga completada (deduplicando por enlace).
    pub async fn record(&self, app: &AppHandle, entry: HistoryEntry) {
        let mut entries = self.0.lock().await;
        upsert(&mut entries, entry);
        let _ = save(app, &entries);
    }

    pub async fn list(&self) -> Vec<HistoryEntry> {
        self.0.lock().await.clone()
    }

    pub async fn remove(&self, app: &AppHandle, url: &str) {
        let mut entries = self.0.lock().await;
        entries.retain(|entry| entry.url != url);
        let _ = save(app, &entries);
    }

    pub async fn clear(&self, app: &AppHandle) {
        let mut entries = self.0.lock().await;
        entries.clear();
        let _ = save(app, &entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(url: &str) -> HistoryEntry {
        HistoryEntry {
            url: url.to_string(),
            title: url.to_string(),
            thumbnail: None,
            file_path: None,
            options: DownloadOptions::default(),
            size_bytes: None,
            finished_at: 0,
        }
    }

    #[test]
    fn inserta_al_frente() {
        let mut entries = vec![entry("a")];
        upsert(&mut entries, entry("b"));
        assert_eq!(entries[0].url, "b");
        assert_eq!(entries[1].url, "a");
    }

    #[test]
    fn deduplica_por_enlace() {
        let mut entries = vec![entry("a"), entry("b")];
        upsert(&mut entries, entry("a"));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].url, "a");
    }

    #[test]
    fn recorta_el_maximo_de_entradas() {
        let mut entries: Vec<HistoryEntry> =
            (0..MAX_ENTRIES).map(|i| entry(&format!("u{i}"))).collect();
        upsert(&mut entries, entry("nueva"));
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert_eq!(entries[0].url, "nueva");
    }
}
