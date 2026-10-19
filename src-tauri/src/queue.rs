use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::sync::Mutex;

use crate::history::{HistoryEntry, HistoryState};
use crate::settings::{Settings, SettingsState};
use crate::ytdlp::{self, DownloadOptions};

/// Canal con la lista completa de trabajos tras cada cambio de estado.
pub const QUEUE_EVENT: &str = "queue-updated";

/// Intervalo mínimo entre emisiones de progreso (evita saturar el frontend).
const EMIT_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn is_active(self) -> bool {
        matches!(self, Self::Downloading | Self::Processing)
    }
}

/// Vista serializable de un trabajo para el frontend y para `queue.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub thumbnail: Option<String>,
    pub options: DownloadOptions,
    pub status: JobStatus,
    pub percent: Option<f64>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub file_path: Option<String>,
    pub message: Option<String>,
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub stream: Option<String>,
}

impl JobView {
    pub fn new(
        id: u64,
        url: String,
        title: String,
        thumbnail: Option<String>,
        options: DownloadOptions,
    ) -> Self {
        Self {
            id,
            url,
            title,
            thumbnail,
            options,
            status: JobStatus::Queued,
            percent: None,
            downloaded_bytes: None,
            total_bytes: None,
            speed_bps: None,
            eta_secs: None,
            file_path: None,
            message: None,
            phase: None,
            stream: None,
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
    /// Aggregates progress across multiple streams (video + audio).
    meters: HashMap<u64, ytdlp::ProgressMeter>,
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

/// Reencola un trabajo fallido o cancelado para volver a intentarlo con las
/// mismas opciones. Devuelve `true` si el trabajo quedó pendiente.
fn requeue_finished(jobs: &mut [JobView], id: u64) -> bool {
    let Some(job) = jobs.iter_mut().find(|j| j.id == id) else {
        return false;
    };
    if !matches!(job.status, JobStatus::Error | JobStatus::Cancelled) {
        return false;
    }
    job.status = JobStatus::Queued;
    job.percent = None;
    job.downloaded_bytes = None;
    job.total_bytes = None;
    job.speed_bps = None;
    job.eta_secs = None;
    job.file_path = None;
    job.message = None;
    job.phase = None;
    job.stream = None;
    true
}

/// Trabajos que se conservan entre sesiones: los que aún no han terminado.
/// Los que estaban en curso vuelven como pendientes, para que yt-dlp reanude
/// el archivo `.part`; los pausados se quedan pausados.
fn restorable_jobs(jobs: &[JobView]) -> Vec<JobView> {
    jobs.iter()
        .filter(|job| !job.status.is_finished())
        .map(|job| {
            let mut view = job.clone();
            view.status = match view.status {
                JobStatus::Paused => JobStatus::Paused,
                _ => JobStatus::Queued,
            };
            view.percent = None;
            view.downloaded_bytes = None;
            view.total_bytes = None;
            view.speed_bps = None;
            view.eta_secs = None;
            view.file_path = None;
            view.message = None;
            view.phase = None;
            view.stream = None;
            view
        })
        .collect()
}

/// Siguiente id libre: el mayor de los trabajos restaurados o el guardado.
fn next_id_from(jobs: &[JobView], stored: u64) -> u64 {
    jobs.iter().map(|job| job.id).max().unwrap_or(0).max(stored)
}

/// Sanity check for a finished download: the file must be where yt-dlp said.
/// Returns the message to show when it is not, or `None` when all is well.
fn missing_output(path: &str) -> Option<String> {
    if std::path::Path::new(path).is_file() {
        return None;
    }
    // Un `content://` de `MediaStore` no es una ruta del sistema: existe, pero
    // `is_file()` no puede verlo. Se da por bueno sin más comprobación.
    if path.starts_with("content://") {
        return None;
    }
    Some(format!(
        "La descarga terminó, pero el archivo no está en «{path}». \
         Comprueba la carpeta de descargas y vuelve a intentarlo."
    ))
}

/// Publica el archivo en la galería (solo Android) y devuelve su `content://`.
///
/// En el escritorio no hay nada que hacer: el archivo ya está en una carpeta que
/// el usuario ve. En Android la descarga vive en `Android/data/<paquete>/files/`,
/// que está oculta, así que se copia a `MediaStore` y se pasa a usar el URI.
fn publish_if_android(path: Option<String>) -> Option<String> {
    let path = path?;
    #[cfg(target_os = "android")]
    {
        if let Some(uri) = crate::media_store::publish(&path) {
            return Some(uri);
        }
    }
    Some(path)
}

/// Borra los acompañantes que yt-dlp deja junto a una descarga ya publicada.
///
/// Al publicar en la galería solo se mueve el archivo principal: los subtítulos
/// sueltos (`.srt`), las miniaturas que no se pudieron incrustar (`.webp`,
/// `.jpg`) y los fragmentos (`.part`, `.ytdl`) se quedaban en la carpeta privada
/// para siempre, engordándola con cada descarga. Aquí se limpian una vez que el
/// archivo bueno ya está en la galería. Solo se usa en Android: en el escritorio
/// el usuario ve y gestiona sus propios archivos.
#[cfg(target_os = "android")]
fn cleanup_companions(main_path: &str) {
    let main = std::path::Path::new(main_path);
    let (Some(dir), Some(stem)) = (main.parent(), main.file_name()) else {
        return;
    };
    let stem = stem.to_string_lossy();
    // El nombre completo del principal; se compara por prefijo para no borrar
    // archivos de otras descargas que compartan el mismo título.
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == *stem {
            continue;
        }
        // Solo acompañantes del mismo prefijo con extensión conocida.
        let Some(rest) = name.strip_prefix(stem.as_ref()) else {
            continue;
        };
        let rest = rest.trim_start_matches('.');
        let is_companion = rest == "part"
            || rest == "ytdl"
            || rest.ends_with(".part")
            || rest.ends_with(".ytdl")
            || rest.ends_with(".srt")
            || rest.ends_with(".vtt")
            || rest.ends_with(".ass")
            || rest.ends_with(".webp")
            || rest.ends_with(".jpg")
            || rest.ends_with(".png");
        if is_companion {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Resume la cola para la notificación del servicio en primer plano.
///
/// Devuelve `None` cuando no hay nada activo (y hay que retirar la
/// notificación), o el texto y el porcentaje que se muestran. Recibe el estado de
/// cada trabajo como `(activo, terminado, porcentaje, título)` para poder probarlo
/// sin montar toda la cola.
///
/// El porcentaje es la **media** de lo que está activo: con varias descargas, la
/// barra refleja el conjunto y no solo la primera (que saltaría atrás cada vez
/// que una termina). El texto nombra la primera descarga activa y, si hay más
/// esperando, lo dice.
fn foreground_summary<I>(jobs: I) -> Option<(String, i32)>
where
    I: IntoIterator<Item = (bool, bool, Option<f64>, String)>,
{
    let mut active = 0usize;
    let mut pending = 0usize;
    let mut percent_sum = 0.0f64;
    let mut first: Option<String> = None;
    for (is_active, is_finished, percent, title) in jobs {
        if is_active {
            active += 1;
            percent_sum += percent.unwrap_or(0.0);
            if first.is_none() && !title.trim().is_empty() {
                first = Some(title);
            }
        }
        if !is_finished {
            pending += 1;
        }
    }
    if active == 0 {
        return None;
    }
    // Media redondeada de lo activo (los porcentajes van de 0 a 100).
    let percent = (percent_sum / active as f64).round() as i32;
    // El título puede no llegar aún (yt-dlp lo resuelve al empezar); si no hay,
    // se cuenta sin más. Cuando hay varios en cola se añade el total.
    let base = first.unwrap_or_else(|| format!("{active} descargas"));
    let text = if pending > active {
        format!("{base} · {pending} en total")
    } else {
        base
    };
    Some((text, percent))
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
                meters: HashMap::new(),
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
        thumbnail: Option<String>,
        options: DownloadOptions,
    ) -> u64 {
        let id = {
            let mut inner = self.inner.lock().await;
            inner.next_id += 1;
            let id = inner.next_id;
            inner
                .jobs
                .push(JobView::new(id, url, title, thumbnail, options));
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
            job.phase = None;
            job.stream = None;
            if let Some(child) = inner.running.get_mut(&id) {
                let _ = child.start_kill();
            }
        }
        self.emit().await;
        self.persist().await;
        // Pausar la última descarga activa debe retirar la notificación.
        self.sync_foreground().await;
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

    /// Cancela un trabajo (en curso o pendiente). Los que ya terminaron se
    /// quitan de la lista; si su proceso aún está drenando, se espera a que
    /// termine (su hueco se libera en `finish`).
    pub async fn cancel(&self, id: u64) {
        {
            let mut inner = self.inner.lock().await;
            let Some(pos) = inner.jobs.iter().position(|j| j.id == id) else {
                return;
            };
            if inner.jobs[pos].status.is_finished() {
                if inner.running.contains_key(&id) {
                    return;
                }
                inner.jobs.remove(pos);
                inner.meters.remove(&id);
            } else {
                let job = &mut inner.jobs[pos];
                job.status = JobStatus::Cancelled;
                job.speed_bps = None;
                job.eta_secs = None;
                job.phase = None;
                job.stream = None;
                if let Some(child) = inner.running.get_mut(&id) {
                    let _ = child.start_kill();
                }
            }
        }
        self.emit().await;
        self.persist().await;
        self.sync_foreground().await;
    }

    /// Reintenta un trabajo fallido o cancelado conservando sus opciones.
    /// Si su proceso todavía está vivo (pausa/cancelación reciente) no hace nada.
    pub async fn retry(self: &Arc<Self>, settings: &Settings, id: u64) {
        {
            let mut inner = self.inner.lock().await;
            if inner.running.contains_key(&id) {
                return;
            }
            requeue_finished(&mut inner.jobs, id);
            inner.meters.remove(&id);
        }
        self.pump(settings.clone()).await;
    }

    /// Reordena los trabajos pendientes según `order`.
    pub async fn reorder(&self, order: &[u64]) {
        let changed = {
            let mut inner = self.inner.lock().await;
            apply_pending_order(&mut inner.jobs, order)
        };
        if changed {
            self.emit().await;
            self.persist().await;
        }
    }

    /// Quita de la cola los trabajos terminados.
    pub async fn clear_finished(&self) {
        {
            let mut inner = self.inner.lock().await;
            let before = inner.jobs.len();
            inner.jobs.retain(|j| !j.status.is_finished());
            let alive: std::collections::HashSet<u64> = inner.jobs.iter().map(|j| j.id).collect();
            inner.meters.retain(|id, _| alive.contains(id));
            if inner.jobs.len() == before {
                return;
            }
        }
        self.emit().await;
        self.persist().await;
    }

    /// Arranca pendientes hasta el límite de concurrencia.
    pub async fn pump(self: &Arc<Self>, settings: Settings) {
        let out_dir = crate::settings::download_dir(&self.app, &settings);
        let mut runners: Vec<(u64, ChildStdout, ChildStderr)> = Vec::new();

        {
            let mut inner = self.inner.lock().await;
            let concurrency = inner.concurrency.max(1);

            while inner.running.len() < concurrency {
                let Some(pos) = next_queued_position(&inner.jobs) else {
                    break;
                };

                let dir = match &out_dir {
                    Ok(dir) => dir,
                    Err(message) => {
                        inner.jobs[pos].status = JobStatus::Error;
                        inner.jobs[pos].message = Some(message.clone());
                        continue;
                    }
                };

                let id = inner.jobs[pos].id;
                let url = inner.jobs[pos].url.clone();
                let options = inner.jobs[pos].options.clone();

                match ytdlp::spawn_process(&settings, &url, &options, dir) {
                    Ok((child, stdout, stderr)) => {
                        inner.jobs[pos].status = JobStatus::Downloading;
                        inner.jobs[pos].message = None;
                        inner.jobs[pos].phase = None;
                        inner.jobs[pos].stream = None;
                        inner.meters.insert(id, ytdlp::ProgressMeter::default());
                        inner.running.insert(id, child);
                        runners.push((id, stdout, stderr));
                    }
                    Err(error) => {
                        inner.jobs[pos].status = JobStatus::Error;
                        inner.jobs[pos].message = Some(error);
                        inner.meters.remove(&id);
                    }
                }
            }
        }

        self.emit().await;
        self.persist().await;

        for (id, stdout, stderr) in runners {
            self.clone().run_job(id, stdout, stderr);
        }

        // Arranca el servicio en primer plano (o lo para si ya no hay nada).
        self.sync_foreground().await;
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
                    Some(ytdlp::LineEvent::Plan(total)) => self.set_plan(id, total).await,
                    Some(ytdlp::LineEvent::File(path)) => final_path = Some(path),
                    Some(ytdlp::LineEvent::Phase(name)) => {
                        self.set_processing(id, Some(name)).await
                    }
                    None => {}
                }
            }

            let _ = stderr_task.await;
            let stderr_text = stderr_buf.lock().await.clone();
            self.finish(id, final_path, &stderr_text).await;
        });
    }

    /// Keeps the total size yt-dlp announces before starting (video + audio).
    async fn set_plan(&self, id: u64, total: u64) {
        let mut inner = self.inner.lock().await;
        inner.meters.entry(id).or_default().plan(total);
    }

    async fn update_progress(&self, id: u64, tick: ytdlp::ProgressTick) {
        let should_emit = {
            let mut inner = self.inner.lock().await;

            // Aggregate first (video + audio are two separate downloads), then
            // look up the job: keeping both borrows apart pleases the checker.
            let (aggregated, stream) = {
                let meter = inner.meters.entry(id).or_default();
                (meter.feed(tick), meter.stream_label().map(str::to_string))
            };

            let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) else {
                return;
            };
            if job.status.is_finished() || job.status == JobStatus::Paused {
                return;
            }
            // A late progress line must not undo the postprocessing state.
            if job.status != JobStatus::Processing {
                job.status = JobStatus::Downloading;
            }
            job.percent = aggregated.percent;
            job.downloaded_bytes = aggregated.downloaded_bytes;
            job.total_bytes = aggregated.total_bytes;
            job.speed_bps = aggregated.speed_bps;
            job.eta_secs = aggregated.eta_secs;
            job.stream = stream;
            job.phase = None;

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
            self.sync_foreground().await;
        }
    }

    async fn set_processing(&self, id: u64, phase: Option<String>) {
        let changed = {
            let mut inner = self.inner.lock().await;
            match inner.jobs.iter_mut().find(|j| j.id == id) {
                Some(job) if !job.status.is_finished() && job.status != JobStatus::Paused => {
                    job.status = JobStatus::Processing;
                    job.percent = Some(100.0);
                    job.speed_bps = None;
                    job.eta_secs = None;
                    job.stream = None;
                    // Later phases (merge, metadata, thumbnail…) refresh the label.
                    job.phase = phase;
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
            inner.meters.remove(&id);
            let status = inner.jobs.iter().find(|j| j.id == id).map(|j| j.status);
            (child, status)
        };

        let success = match child.as_mut() {
            Some(child) => child.wait().await.map(|s| s.success()).unwrap_or(false),
            None => false,
        };
        drop(child);

        // 1.5) En Android, pasa el archivo a la galería y quédate con su URI.
        //
        // La descarga nace en `Android/data/<paquete>/files/`, que el usuario no
        // ve. Publicarla en `MediaStore` hace que aparezca en la galería, y a
        // partir de ahí es el `content://` lo que se guarda como ruta: es lo que
        // sabe abrir el sistema. Si la publicación falla, se conserva la ruta
        // real y la descarga sigue siendo válida.
        //
        // Se hace fuera del lock porque copiar un vídeo es lento (y el lock lo
        // comparten la cola y la interfaz).
        let final_path = if success
            && !matches!(
                previous,
                Some(JobStatus::Paused) | Some(JobStatus::Cancelled)
            )
        {
            // En Android, tras publicar, se limpian los acompañantes (subtítulos
            // sueltos, miniaturas, fragmentos) que si no se acumulan en la carpeta
            // privada. En el escritorio no se toca nada: ahí el usuario ve y
            // gestiona sus archivos.
            let published = publish_if_android(final_path.clone());
            #[cfg(target_os = "android")]
            if let Some(original) = final_path.as_deref() {
                cleanup_companions(original);
            }
            published
        } else {
            final_path
        };

        // 2) Fija el estado final (salvo si un comando ya lo dejó terminal).
        let mut completed: Option<HistoryEntry> = None;
        let mut notify_info: Option<(String, bool)> = None;
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
                // The download is only done if the file yt-dlp reported is there.
                match final_path.as_deref().and_then(missing_output) {
                    Some(problem) => {
                        job.status = JobStatus::Error;
                        job.message = Some(problem);
                        notify_info = Some((job.title.clone(), false));
                    }
                    None => {
                        job.status = JobStatus::Done;
                        job.percent = Some(100.0);
                        job.file_path = final_path;
                        if job.file_path.is_none() {
                            job.message = Some(
                                "La descarga terminó, pero yt-dlp no indicó la ruta del archivo. \
                                 Revisa la carpeta de descargas."
                                    .to_string(),
                            );
                        }
                        let file_path = job.file_path.clone();
                        completed = Some(HistoryEntry::new(
                            job.url.clone(),
                            job.title.clone(),
                            job.thumbnail.clone(),
                            file_path,
                            job.options.clone(),
                        ));
                        notify_info = Some((job.title.clone(), true));
                    }
                }
                job.speed_bps = None;
                job.eta_secs = None;
                job.phase = None;
                job.stream = None;
            } else {
                job.status = JobStatus::Error;
                job.speed_bps = None;
                job.eta_secs = None;
                job.phase = None;
                job.stream = None;
                job.message = Some(ytdlp::friendly_error(stderr_text));
                notify_info = Some((job.title.clone(), false));
            }
        }

        // 3) Guarda la descarga en el historial.
        if let Some(entry) = completed
            && let Some(state) = self.app.try_state::<HistoryState>()
        {
            state.record(&self.app, entry).await;
        }

        // 4) Avisa al sistema si la ventana no está delante.
        if let Some((title, ok)) = notify_info {
            self.notify_finished(title, ok).await;
        }

        self.emit().await;

        // 5) Hueco libre: intenta arrancar el siguiente pendiente.
        let settings = self.current_settings().await;
        self.pump(settings).await;
    }

    /// Recupera los trabajos guardados de la sesión anterior.
    pub async fn restore(self: &Arc<Self>, settings: &Settings) {
        if !settings.persist_queue {
            return;
        }
        let Some(stored) = crate::queue_store::load(&self.app) else {
            return;
        };
        let jobs = restorable_jobs(&stored.jobs);
        if jobs.is_empty() {
            return;
        }
        {
            let mut inner = self.inner.lock().await;
            inner.next_id = next_id_from(&jobs, stored.next_id);
            inner.jobs = jobs;
        }
        self.emit().await;
        self.pump(settings.clone()).await;
    }

    /// Sincroniza el servicio en primer plano con lo que hay en la cola.
    ///
    /// Android mata o congela la app en cuanto pierde el foco, y con ella la
    /// descarga. Mientras haya algún trabajo activo se mantiene un *foreground
    /// service* (con su notificación obligatoria, que de paso muestra el
    /// progreso); cuando no queda ninguno, se retira. Se llama desde los puntos
    /// donde cambia ese estado —arranque, progreso, fase y cierre— y es
    /// idempotente: repetir el estado actual no cuesta nada.
    async fn sync_foreground(&self) {
        let state = {
            let inner = self.inner.lock().await;
            foreground_summary(inner.jobs.iter().map(|job| {
                (
                    job.status.is_active(),
                    job.status.is_finished(),
                    job.percent,
                    job.title.clone(),
                )
            }))
        };

        #[cfg(target_os = "android")]
        {
            match state {
                None => crate::foreground::stop(),
                Some((text, percent)) => crate::foreground::update(&text, percent),
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = state;
    }

    /// Muestra una notificación del sistema al terminar una descarga (solo si
    /// está activada y la ventana no tiene el foco: si está delante, ya se ve).
    async fn notify_finished(&self, title: String, success: bool) {
        let settings = self.current_settings().await;
        if !settings.notify_on_finish {
            return;
        }
        // En escritorio, si la ventana está delante no hace falta avisar. En
        // Android la app puede estar en segundo plano sin que `is_focused` lo
        // diga, así que se avisa siempre: es justo lo que el usuario espera.
        #[cfg(desktop)]
        if let Some(window) = self.app.get_webview_window("main")
            && window.is_focused().unwrap_or(false)
        {
            return;
        }
        use tauri_plugin_notification::NotificationExt;
        let heading = if success {
            "Descarga completada"
        } else {
            "La descarga falló"
        };
        let _ = self
            .app
            .notification()
            .builder()
            .title(heading)
            .body(title)
            .show();
    }

    /// Guarda en disco los trabajos pendientes (si está activado en Ajustes).
    async fn persist(&self) {
        let settings = self.current_settings().await;
        if !settings.persist_queue {
            return;
        }
        let queue = {
            let inner = self.inner.lock().await;
            crate::queue_store::PersistedQueue {
                next_id: inner.next_id,
                jobs: restorable_jobs(&inner.jobs),
            }
        };
        let _ = crate::queue_store::save(&self.app, &queue);
    }

    async fn current_settings(&self) -> Settings {
        self.app.state::<SettingsState>().0.lock().await.clone()
    }

    async fn emit(&self) {
        let jobs = self.inner.lock().await.jobs.clone();
        let _ = self.app.emit(QUEUE_EVENT, &jobs);
        #[cfg(desktop)]
        crate::tray::sync_tooltip(&self.app, &jobs);
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
            None,
            Default::default(),
        );
        view.status = status;
        view
    }

    /// Sin trabajos activos no hay notificación que mostrar.
    #[test]
    fn foreground_sin_activos_no_hay_notificacion() {
        let jobs = vec![
            (false, true, Some(100.0), "hecho".to_string()),
            (false, true, Some(0.0), "cancelado".to_string()),
        ];
        assert!(foreground_summary(jobs).is_none());
        assert!(foreground_summary(Vec::new()).is_none());
    }

    /// Con una sola descarga, el texto es su título y el porcentaje el suyo.
    #[test]
    fn foreground_una_descarga_usa_su_titulo() {
        let jobs = vec![(true, false, Some(42.6), "Un vídeo".to_string())];
        let (text, percent) = foreground_summary(jobs).expect("hay una activa");
        assert_eq!(text, "Un vídeo");
        assert_eq!(percent, 43);
    }

    /// El porcentaje es la media de lo activo, no el de la primera.
    #[test]
    fn foreground_promedia_el_progreso() {
        let jobs = vec![
            (true, false, Some(0.0), "A".to_string()),
            (true, false, Some(100.0), "B".to_string()),
        ];
        let (_, percent) = foreground_summary(jobs).expect("hay activas");
        assert_eq!(percent, 50);
    }

    /// Si hay más trabajos esperando que activos, se anuncia el total.
    #[test]
    fn foreground_anuncia_la_cola() {
        let jobs = vec![
            (true, false, Some(10.0), "En curso".to_string()),
            (false, false, None, "En espera".to_string()),
        ];
        let (text, _) = foreground_summary(jobs).expect("hay activas");
        assert_eq!(text, "En curso · 2 en total");
    }

    /// Un trabajo activo sin título todavía (yt-dlp no lo ha resuelto) no
    /// produce un texto vacío.
    #[test]
    fn foreground_sin_titulo_no_queda_vacio() {
        let jobs = vec![
            (true, false, None, String::new()),
            (true, false, None, String::new()),
        ];
        let (text, percent) = foreground_summary(jobs).expect("hay activas");
        assert_eq!(text, "2 descargas");
        // Sin porcentaje conocido las descargas cuentan como 0.
        assert_eq!(percent, 0);
    }

    #[test]
    fn next_queued_ignora_estados_no_pendientes() {        let jobs = vec![
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
    fn reencolar_reintenta_fallidos_y_cancelados() {
        let mut failed = job(1, JobStatus::Error);
        failed.message = Some("boom".to_string());
        let cancelled = job(2, JobStatus::Cancelled);
        let mut done = job(3, JobStatus::Done);
        done.file_path = Some("/tmp/x.mp4".to_string());
        let mut jobs = vec![failed, cancelled, done, job(4, JobStatus::Queued)];

        assert!(requeue_finished(&mut jobs, 1));
        assert_eq!(jobs[0].status, JobStatus::Queued);
        assert_eq!(jobs[0].message, None);

        assert!(requeue_finished(&mut jobs, 2));
        assert_eq!(jobs[1].status, JobStatus::Queued);

        // Un trabajo completado no se reintenta ni pierde su ruta.
        assert!(!requeue_finished(&mut jobs, 3));
        assert_eq!(jobs[2].status, JobStatus::Done);
        assert_eq!(jobs[2].file_path.as_deref(), Some("/tmp/x.mp4"));

        // Ids desconocidos o ya pendientes no cambian nada.
        assert!(!requeue_finished(&mut jobs, 99));
        assert!(!requeue_finished(&mut jobs, 4));
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
        assert!(JobStatus::Downloading.is_active());
        assert!(JobStatus::Processing.is_active());
        assert!(!JobStatus::Queued.is_active());
    }

    #[test]
    fn restorable_conserva_pendientes_y_pausados_y_limpia_el_progreso() {
        let mut active = job(1, JobStatus::Downloading);
        active.percent = Some(40.0);
        active.downloaded_bytes = Some(1024);
        active.file_path = Some("/tmp/parcial.mp4".to_string());
        active.message = Some("no debería quedar".to_string());
        let paused = job(2, JobStatus::Paused);
        let queued = job(3, JobStatus::Queued);
        let done = job(4, JobStatus::Done);
        let failed = job(5, JobStatus::Error);
        let cancelled = job(6, JobStatus::Cancelled);

        let jobs = vec![active, paused, queued, done, failed, cancelled];
        let saved = restorable_jobs(&jobs);

        let ids: Vec<u64> = saved.iter().map(|j| j.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
        // El trabajo en curso vuelve como pendiente y sin progreso heredado.
        assert_eq!(saved[0].status, JobStatus::Queued);
        assert_eq!(saved[0].percent, None);
        assert_eq!(saved[0].downloaded_bytes, None);
        assert_eq!(saved[0].file_path, None);
        assert_eq!(saved[0].message, None);
        // El pausado se queda pausado.
        assert_eq!(saved[1].status, JobStatus::Paused);
        assert_eq!(saved[2].status, JobStatus::Queued);
    }

    #[test]
    fn siguiente_id_toma_el_mayor() {
        let jobs = vec![job(2, JobStatus::Queued), job(7, JobStatus::Paused)];
        assert_eq!(next_id_from(&jobs, 3), 7);
        assert_eq!(next_id_from(&jobs, 20), 20);
        assert_eq!(next_id_from(&[], 9), 9);
    }

    #[test]
    fn restorable_ignora_una_cola_ya_terminada() {
        let jobs = vec![job(1, JobStatus::Done), job(2, JobStatus::Cancelled)];
        assert!(restorable_jobs(&jobs).is_empty());
    }
}
