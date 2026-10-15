use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::sync::Mutex;

use crate::settings::{Settings, SettingsState};
use crate::ytdlp::{self, DownloadOptions};

/// Canal con la lista completa de trabajos tras cada cambio de estado.
pub const QUEUE_EVENT: &str = "queue-updated";

/// Intervalo mínimo entre emisiones de progreso (evita saturar el frontend).
const EMIT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Downloading,
    Processing,
    Paused,
    Done,
    Error,
    Cancelled,
}

impl JobStatus {
    /// ¿Se puede pausar? Solo mientras descarga o procesa.
    pub fn can_pause(self) -> bool {
        matches!(self, Self::Downloading | Self::Processing)
    }

    /// ¿Se puede reanudar?
    pub fn can_resume(self) -> bool {
        matches!(self, Self::Paused)
    }

    /// ¿Ya terminó (con éxito o no)?
    pub fn is_finished(self) -> bool {
        matches!(self, Self::Done | Self::Error | Self::Cancelled)
    }
}

/// Vista serializable de un trabajo para el frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub options: DownloadOptions,
    pub status: JobStatus,
    pub percent: Option<f64>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub file_path: Option<String>,
    pub message: Option<String>,
}

impl JobView {
    fn new(id: u64, url: String, title: String, options: DownloadOptions) -> Self {
        Self {
            id,
            url,
            title,
            options,
            status: JobStatus::Queued,
            percent: None,
            downloaded_bytes: None,
            total_bytes: None,
            speed_bps: None,
            eta_secs: None,
            file_path: None,
            message: None,
        }
    }
}

struct Inner {
    /// Orden del vector = orden de despacho de los pendientes.
    jobs: Vec<JobView>,
    /// Procesos en marcha, por id de trabajo.
    running: HashMap<u64, Child>,
    next_id: u64,
    concurrency: usize,
    last_emit: Option<Instant>,
}

/// Gestor de la cola de descargas.
pub struct Queue {
    inner: Mutex<Inner>,
    app: AppHandle,
}

/// Posición del primer trabajo pendiente en la cola (función pura, testeable).
fn next_queued_position(jobs: &[JobView]) -> Option<usize> {
    jobs.iter().position(|j| j.status == JobStatus::Queued)
}

/// Reordena los pendientes según `order` y los coloca al frente de la cola.
/// Los trabajos que no estén en `order` conservan su posición relativa al final
/// de los pendientes. Devuelve `true` si hubo algún cambio de orden.
fn apply_pending_order(jobs: &mut Vec<JobView>, order: &[u64]) -> bool {
    let mut pending: Vec<JobView> = Vec::new();
    let mut rest: Vec<JobView> = Vec::new();
    for job in jobs.drain(..) {
        if job.status == JobStatus::Queued {
            pending.push(job);
        } else {
            rest.push(job);
        }
    }

    let before: Vec<u64> = pending.iter().map(|j| j.id).collect();

    let mut ordered: Vec<JobView> = Vec::new();
    for id in order {
        if let Some(pos) = pending.iter().position(|j| j.id == *id) {
            ordered.push(pending.remove(pos));
        }
    }
    ordered.extend(pending);
    ordered.extend(rest);

    let after: Vec<u64> = ordered
        .iter()
        .filter(|j| j.status == JobStatus::Queued)
        .map(|j| j.id)
        .collect();
    let changed = before != after;

    *jobs = ordered;
    changed
}

impl Queue {
    pub fn new(app: AppHandle, concurrency: usize) -> Self {
        Self {
            inner: Mutex::new(Inner {
                jobs: Vec::new(),
                running: HashMap::new(),
                next_id: 0,
                concurrency: concurrency.clamp(1, 8),
                last_emit: None,
            }),
            app,
        }
    }

    pub async fn set_concurrency(&self, value: usize) {
        self.inner.lock().await.concurrency = value.clamp(1, 8);
    }

    pub async fn snapshot(&self) -> Vec<JobView> {
        self.inner.lock().await.jobs.clone()
    }

    /// Añade un enlace a la cola y arranca lo que permita la concurrencia.
    pub async fn enqueue(
        self: &Arc<Self>,
        settings: &Settings,
        url: String,
        title: String,
        options: DownloadOptions,
    ) -> u64 {
        let id = {
            let mut inner = self.inner.lock().await;
            inner.next_id += 1;
            let id = inner.next_id;
            inner.jobs.push(JobView::new(id, url, title, options));
            id
        };
        self.pump(settings.clone()).await;
        id
    }

    /// Pausa un trabajo en curso (mata el proceso; yt-dlp reanuda al reanudar).
    pub async fn pause(&self, id: u64) {
        {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if !job.status.can_pause() {
                return;
            }
            job.status = JobStatus::Paused;
            job.speed_bps = None;
            job.eta_secs = None;
            if let Some(child) = inner.running.get_mut(&id) {
                let _ = child.start_kill();
            }
        }
        self.emit().await;
    }

    /// Reanuda un trabajo pausado.
    pub async fn resume(self: &Arc<Self>, settings: &Settings, id: u64) {
        {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if !job.status.can_resume() {
                return;
            }
            job.status = JobStatus::Queued;
            job.message = None;
        }
        self.pump(settings.clone()).await;
    }

    /// Cancela un trabajo (en curso o pendiente).
    pub async fn cancel(&self, id: u64) {
        {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if job.status.is_finished() {
                return;
            }
            job.status = JobStatus::Cancelled;
            job.speed_bps = None;
            job.eta_secs = None;
            if let Some(child) = inner.running.get_mut(&id) {
                let _ = child.start_kill();
            }
        }
        self.emit().await;
    }

    /// Reordena los trabajos pendientes según `order`.
    pub async fn reorder(&self, order: &[u64]) {
        let changed = {
            let mut inner = self.inner.lock().await;
            apply_pending_order(&mut inner.jobs, order)
        };
        if changed {
            self.emit().await;
        }
    }

    /// Quita de la cola los trabajos terminados.
    pub async fn clear_finished(&self) {
        {
            let mut inner = self.inner.lock().await;
            let before = inner.jobs.len();
            inner.jobs.retain(|j| !j.status.is_finished());
            if inner.jobs.len() == before {
                return;
            }
        }
        self.emit().await;
    }

    /// Arranca pendientes hasta el límite de concurrencia.
    pub async fn pump(self: &Arc<Self>, settings: Settings) {
        let out_dir = self.app.path().download_dir().ok();
        let mut runners: Vec<(u64, ChildStdout, ChildStderr)> = Vec::new();

        {
            let mut inner = self.inner.lock().await;
            let concurrency = inner.concurrency.max(1);

            while inner.running.len() < concurrency {
                let Some(pos) = next_queued_position(&inner.jobs) else {
                    break;
                };

                let Some(dir) = out_dir.as_deref() else {
                    inner.jobs[pos].status = JobStatus::Error;
                    inner.jobs[pos].message =
                        Some("No se encontró la carpeta de Descargas.".to_string());
                    continue;
                };

                let id = inner.jobs[pos].id;
                let url = inner.jobs[pos].url.clone();
                let options = inner.jobs[pos].options.clone();

                match ytdlp::spawn_process(&settings, &url, &options, dir) {
                    Ok((child, stdout, stderr)) => {
                        inner.jobs[pos].status = JobStatus::Downloading;
                        inner.jobs[pos].message = None;
                        inner.running.insert(id, child);
                        runners.push((id, stdout, stderr));
                    }
                    Err(error) => {
                        inner.jobs[pos].status = JobStatus::Error;
                        inner.jobs[pos].message = Some(error);
                    }
                }
            }
        }

        self.emit().await;

        for (id, stdout, stderr) in runners {
            self.clone().run_job(id, stdout, stderr);
        }
    }

    /// Lee la salida de un trabajo y actualiza su estado.
    fn run_job(self: Arc<Self>, id: u64, stdout: ChildStdout, stderr: ChildStderr) {
        tauri::async_runtime::spawn(async move {
            // Drena stderr para poder reportar el error final.
            let stderr_buf = Arc::new(Mutex::new(String::new()));
            let buf = stderr_buf.clone();
            let stderr_task = tauri::async_runtime::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut text = buf.lock().await;
                    text.push_str(&line);
                    text.push('\n');
                }
            });

            let mut final_path: Option<String> = None;
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match ytdlp::parse_line(&line) {
                    Some(ytdlp::LineEvent::Progress(tick)) => self.update_progress(id, tick).await,
                    Some(ytdlp::LineEvent::File(path)) => final_path = Some(path),
                    Some(ytdlp::LineEvent::Phase(_)) => self.set_processing(id).await,
                    None => {}
                }
            }

            let _ = stderr_task.await;
            let stderr_text = stderr_buf.lock().await.clone();
            self.finish(id, final_path, &stderr_text).await;
        });
    }

    async fn update_progress(&self, id: u64, tick: ytdlp::ProgressTick) {
        let should_emit = {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if job.status.is_finished() || job.status == JobStatus::Paused {
                return;
            }
            job.status = JobStatus::Downloading;
            job.percent = tick.percent;
            job.downloaded_bytes = tick.downloaded_bytes;
            job.total_bytes = tick.total_bytes;
            job.speed_bps = tick.speed_bps;
            job.eta_secs = tick.eta_secs;

            let now = Instant::now();
            match inner.last_emit {
                Some(prev) if now.duration_since(prev) < EMIT_INTERVAL => false,
                _ => {
                    inner.last_emit = Some(now);
                    true
                }
            }
        };
        if should_emit {
            self.emit().await;
        }
    }

    async fn set_processing(&self, id: u64) {
        let changed = {
            let mut inner = self.inner.lock().await;
            match inner.jobs.iter_mut().find(|j| j.id == id) {
                Some(job)
                    if !job.status.is_finished()
                        && job.status != JobStatus::Paused
                        && job.status != JobStatus::Processing =>
                {
                    job.status = JobStatus::Processing;
                    job.percent = Some(100.0);
                    job.speed_bps = None;
                    job.eta_secs = None;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.emit().await;
        }
    }

    /// Cierra un trabajo y libera su slot para el siguiente pendiente.
    async fn finish(self: &Arc<Self>, id: u64, final_path: Option<String>, stderr_text: &str) {
        // 1) Saca el proceso y recupera el estado actual sin retener el lock.
        let (mut child, previous) = {
            let mut inner = self.inner.lock().await;
            let child = inner.running.remove(&id);
            let status = inner.jobs.iter().find(|j| j.id == id).map(|j| j.status);
            (child, status)
        };

        let success = match child.as_mut() {
            Some(child) => child.wait().await.map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        drop(child);

        // 2) Fija el estado final (salvo si un comando ya lo dejó terminal).
        {
            let mut inner = self.inner.lock().await;
            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if matches!(
                previous,
                Some(JobStatus::Paused) | Some(JobStatus::Cancelled)
            ) {
                // El comando de pausa/cancelación ya fijó el estado; solo liberamos el slot.
            } else if success {
                job.status = JobStatus::Done;
                job.percent = Some(100.0);
                job.speed_bps = None;
                job.eta_secs = None;
                job.file_path = final_path;
            } else {
                job.status = JobStatus::Error;
                job.speed_bps = None;
                job.eta_secs = None;
                job.message = Some(ytdlp::friendly_error(stderr_text));
            }
        }

        self.emit().await;

        // 3) Hueco libre: intenta arrancar el siguiente pendiente.
        let settings = self.current_settings().await;
        self.pump(settings).await;
    }

    async fn current_settings(&self) -> Settings {
        self.app.state::<SettingsState>().0.lock().await.clone()
    }

    async fn emit(&self) {
        let jobs = self.inner.lock().await.jobs.clone();
        let _ = self.app.emit(QUEUE_EVENT, jobs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u64, status: JobStatus) -> JobView {
        let mut view = JobView::new(
            id,
            format!("https://x/{id}"),
            format!("job {id}"),
            Default::default(),
        );
        view.status = status;
        view
    }

    #[test]
    fn next_queued_ignora_estados_no_pendientes() {
        let jobs = vec![
            job(1, JobStatus::Downloading),
            job(2, JobStatus::Done),
            job(3, JobStatus::Queued),
        ];
        assert_eq!(next_queued_position(&jobs), Some(2));
    }

    #[test]
    fn next_queued_devuelve_none_sin_pendientes() {
        let jobs = vec![job(1, JobStatus::Downloading), job(2, JobStatus::Paused)];
        assert_eq!(next_queued_position(&jobs), None);
    }

    #[test]
    fn reordenar_pendientes_respeta_el_orden_pedido() {
        let mut jobs = vec![
            job(1, JobStatus::Queued),
            job(2, JobStatus::Queued),
            job(3, JobStatus::Queued),
        ];
        let changed = apply_pending_order(&mut jobs, &[3, 1, 2]);
        assert!(changed);
        let ids: Vec<u64> = jobs.iter().map(|j| j.id).collect();
        assert_eq!(ids, vec![3, 1, 2]);
    }

    #[test]
    fn reordenar_no_mueve_trabajos_en_curso() {
        let mut jobs = vec![
            job(1, JobStatus::Downloading),
            job(2, JobStatus::Queued),
            job(3, JobStatus::Queued),
        ];
        apply_pending_order(&mut jobs, &[3, 2]);
        // El que descarga permanece, los pendientes van al frente reordenados.
        let ids: Vec<u64> = jobs.iter().map(|j| j.id).collect();
        assert_eq!(ids, vec![3, 2, 1]);
        assert_eq!(jobs[0].status, JobStatus::Queued);
        assert_eq!(jobs[2].status, JobStatus::Downloading);
    }

    #[test]
    fn reordenar_con_ids_desconocidos_los_deja_al_final() {
        let mut jobs = vec![job(1, JobStatus::Queued), job(2, JobStatus::Queued)];
        let changed = apply_pending_order(&mut jobs, &[2, 99]);
        assert!(changed);
        let ids: Vec<u64> = jobs.iter().map(|j| j.id).collect();
        assert_eq!(ids, vec![2, 1]);
    }

    #[test]
    fn reordenar_sin_cambios_no_reporta_cambio() {
        let mut jobs = vec![job(1, JobStatus::Queued), job(2, JobStatus::Queued)];
        assert!(!apply_pending_order(&mut jobs, &[1, 2]));
    }

    #[test]
    fn estados_clasificados_correctamente() {
        assert!(JobStatus::Downloading.can_pause());
        assert!(JobStatus::Processing.can_pause());
        assert!(!JobStatus::Queued.can_pause());
        assert!(JobStatus::Paused.can_resume());
        assert!(JobStatus::Done.is_finished());
        assert!(JobStatus::Error.is_finished());
        assert!(JobStatus::Cancelled.is_finished());
        assert!(!JobStatus::Paused.is_finished());
    }
}
