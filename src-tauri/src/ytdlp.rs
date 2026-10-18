use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::{Child, ChildStderr, ChildStdout};

use crate::engine;
use crate::settings::{CookiesMode, EngineInfo, Settings};

const PROGRESS_TEMPLATE: &str = "download:__DL__%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(info.vcodec)s";
/// `before_dl` is printed once per job and `filesize` is already the sum of the
/// requested formats, so a merge gives the real total before it starts.
const PLAN_TEMPLATE: &str = "before_dl:__PLAN__%(filesize,filesize_approx)s";
const FILE_TEMPLATE: &str = "after_move:__FILE__%(filepath)s";
const JS_RUNTIMES: [(&str, &str); 4] = [
    ("deno", "deno"),
    ("node", "node"),
    ("bun", "bun"),
    ("quickjs", "qjs"),
];

/// Nombre del binario del runtime JS gestionado por la app (quickjs).
const MANAGED_JS_RUNTIME_BIN: &str = "qjs";
const NORMALIZE_FILTER: &str = "loudnorm=I=-16:TP=-1.5:LRA=11";

/// Tope de elementos al listar una radio/mix de YouTube. Son listas
/// autogeneradas que repiten vídeos y pueden ser casi infinitas.
const RADIO_PLAYLIST_LIMIT: usize = 50;

/// Ruta al ejecutable de yt-dlp (variable de entorno → gestionado → sidecar → PATH).
pub fn ytdlp_bin() -> PathBuf {
    engine::ytdlp().0
}

/// Runtime JS resuelto: nombre legible, valor para `--js-runtimes` y origen.
pub struct JsRuntime {
    /// Nombre que yt-dlp reconoce ("node", "quickjs"…), para mostrar en Ajustes.
    pub name: String,
    /// Valor de `--js-runtimes`: `runtime` o `runtime:ruta`.
    pub arg: String,
    pub source: engine::Source,
}

/// Resuelve el runtime JS a usar, en este orden:
///
/// 1. `DOWNLINK_JS_RUNTIME` (override manual).
/// 2. Un runtime del `PATH` (deno, node, bun o quickjs).
/// 3. La copia **gestionada** de quickjs que descarga la app (no está en el
///    `PATH`, así que se le pasa su ruta a yt-dlp).
///
/// Es necesario para resolver los desafíos `n` de YouTube.
pub fn resolve_js_runtime() -> Option<JsRuntime> {
    if let Ok(value) = std::env::var("DOWNLINK_JS_RUNTIME")
        && !value.trim().is_empty()
    {
        let value = value.trim().to_string();
        let name = value.split(':').next().unwrap_or(&value).to_string();
        return Some(JsRuntime {
            name,
            arg: value,
            source: engine::Source::Env,
        });
    }

    for (name, bin) in JS_RUNTIMES {
        if let Some(path) = engine::which(bin) {
            return Some(JsRuntime {
                name: name.to_string(),
                arg: format!("{name}:{}", path.display()),
                source: engine::Source::System,
            });
        }
    }

    if let Some(path) = managed_js_runtime_path() {
        return Some(JsRuntime {
            name: "quickjs".to_string(),
            arg: format!("quickjs:{}", path.display()),
            source: engine::Source::Managed,
        });
    }

    None
}

/// Ruta del runtime JS gestionado (quickjs) si ya está descargado.
pub fn managed_js_runtime_path() -> Option<PathBuf> {
    engine::managed_path(MANAGED_JS_RUNTIME_BIN).filter(|path| path.is_file())
}

/// Valor para `--js-runtimes` (runtime o `runtime:ruta`).
fn js_runtime_arg() -> Option<String> {
    resolve_js_runtime().map(|runtime| runtime.arg)
}

fn is_youtube(url: &str) -> bool {
    let url = url.to_lowercase();
    url.contains("youtube.com") || url.contains("youtu.be")
}

/// ¿Es una «radio»/«mix» autogenerada de YouTube (`list=RD…`, `start_radio=1`)?
/// Esas listas se generan al vuelo, repiten vídeos y pueden no tener fin, así
/// que conviene limitarlas al listarlas.
fn is_youtube_radio(url: &str) -> bool {
    if !is_youtube(url) {
        return false;
    }
    let url = url.to_lowercase();
    url.contains("start_radio=1") || url.contains("list=rd")
}

/// Argumentos comunes a todas las invocaciones de yt-dlp.
fn common_args(settings: &Settings, url: &str) -> Vec<String> {
    let mut args = vec!["--ignore-config".to_string()];

    match settings.cookies_mode {
        CookiesMode::Browser => {
            if let Some(browser) = settings.cookies_browser.as_deref() {
                args.push("--cookies-from-browser".into());
                args.push(browser.to_string());
            }
        }
        CookiesMode::File => {
            if let Some(file) = settings.cookies_file.as_deref() {
                args.push("--cookies".into());
                args.push(file.to_string());
            }
        }
        CookiesMode::None => {}
    }

    if let Some(runtime) = js_runtime_arg() {
        args.push("--js-runtimes".into());
        args.push(runtime);
    }

    // Si ffmpeg no está en el PATH (copia gestionada o sidecar), se lo indicamos.
    if let Some((path, source)) = engine::ffmpeg()
        && source != engine::Source::System
    {
        args.push("--ffmpeg-location".into());
        args.push(path.display().to_string());
    }

    // YouTube exige resolver desafíos JS con el script oficial (EJS).
    if is_youtube(url) {
        args.push("--remote-components".into());
        args.push("ejs:github".into());
    }

    args
}

fn opt_f64(raw: &str) -> Option<f64> {
    let value = raw.trim();
    if value.is_empty() || value == "NA" || value == "None" {
        return None;
    }
    value.parse().ok()
}

// ---------------------------------------------------------------------------
// Opciones de descarga
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MediaMode {
    #[default]
    Video,
    Audio,
}

/// Qué hacer con los segmentos detectados por SponsorBlock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SponsorBlockMode {
    /// Quitar los segmentos del archivo final.
    #[default]
    Remove,
    /// Dejar los segmentos, marcados como capítulos.
    Mark,
}

/// Categorías de SponsorBlock que la app ofrece. Las desconocidas se ignoran
/// antes de llegar a la línea de comandos.
pub const SPONSORBLOCK_CATEGORIES: [&str; 8] = [
    "sponsor",
    "selfpromo",
    "interaction",
    "intro",
    "outro",
    "preview",
    "filler",
    "music_offtopic",
];

/// Categoría que se usa cuando SponsorBlock está activo sin ninguna marcada.
const DEFAULT_SPONSORBLOCK_CATEGORY: &str = "sponsor";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOptions {
    #[serde(default)]
    pub mode: MediaMode,
    /// Altura máxima en píxeles (720, 1080, 2160…). `None` = la mejor disponible.
    pub video_height: Option<u32>,
    /// Contenedor de video: mp4, mkv o webm.
    pub container: Option<String>,
    /// Formato de audio: mp3, m4a, opus, vorbis, flac, wav o "best" (sin recomprimir).
    pub audio_format: Option<String>,
    /// Calidad de audio de conversión: 320K, 256K, 192K, 128K.
    pub audio_quality: Option<String>,
    /// Ajuste de volumen en decibelios (-20 a +20). `None` = sin cambios.
    pub volume_db: Option<f64>,
    /// Normaliza el volumen a -16 LUFS (filtro loudnorm).
    #[serde(default)]
    pub normalize_audio: bool,
    #[serde(default)]
    pub embed_metadata: bool,
    #[serde(default)]
    pub embed_thumbnail: bool,
    /// Recorte: segundo en el que empieza el fragmento.
    #[serde(default)]
    pub clip_start: Option<f64>,
    /// Recorte: segundo en el que termina (None = hasta el final).
    #[serde(default)]
    pub clip_end: Option<f64>,
    /// Descarga los subtítulos del vídeo.
    #[serde(default)]
    pub subtitles: bool,
    /// Idiomas de subtítulos separados por coma (p. ej. "es,en").
    #[serde(default)]
    pub subtitle_langs: Option<String>,
    /// Incluye los subtítulos automáticos (generados por el sitio).
    #[serde(default)]
    pub subtitle_auto: bool,
    /// Incrusta los subtítulos en el vídeo en vez de dejar archivos .srt.
    #[serde(default)]
    pub embed_subtitles: bool,
    /// Quita o marca segmentos promocionales con SponsorBlock (solo YouTube).
    #[serde(default)]
    pub sponsorblock: bool,
    /// Qué hacer con los segmentos: quitar (`remove`) o marcar (`mark`).
    #[serde(default)]
    pub sponsorblock_mode: SponsorBlockMode,
    /// Categorías a tratar. Vacío = la categoría por defecto (`sponsor`).
    #[serde(default)]
    pub sponsorblock_categories: Vec<String>,
}

impl DownloadOptions {
    /// Filtros de ffmpeg (`-af`) derivados de las opciones.
    fn audio_filters(&self) -> String {
        let mut parts = Vec::new();
        if let Some(db) = self.volume_db
            && db.abs() > f64::EPSILON
        {
            parts.push(format!("volume={db}dB"));
        }
        if self.normalize_audio {
            parts.push(NORMALIZE_FILTER.to_string());
        }
        parts.join(",")
    }

    /// ¿Los filtros de audio obligan a re-codificar el stream?
    fn needs_audio_reencode(&self) -> bool {
        !self.audio_filters().is_empty()
    }

    /// Sección de recorte para `--download-sections` (formato `*inicio-fin`).
    fn clip_section(&self) -> Option<String> {
        let start = self.clip_start?;
        let end = self
            .clip_end
            .map(timecode)
            .unwrap_or_else(|| "inf".to_string());
        Some(format!("*{}-{end}", timecode(start)))
    }

    /// Categorías de SponsorBlock listas para la línea de comandos: se quitan
    /// las desconocidas y los repetidos; si no queda ninguna, la de por defecto.
    fn sponsorblock_categories(&self) -> String {
        let mut seen = std::collections::HashSet::new();
        let selected: Vec<String> = self
            .sponsorblock_categories
            .iter()
            .map(|category| category.trim().to_lowercase())
            .filter(|category| SPONSORBLOCK_CATEGORIES.contains(&category.as_str()))
            .filter(|category| seen.insert(category.clone()))
            .collect();

        if selected.is_empty() {
            DEFAULT_SPONSORBLOCK_CATEGORY.to_string()
        } else {
            selected.join(",")
        }
    }
}

/// Convierte segundos a `HH:MM:SS` (formato que acepta yt-dlp).
fn timecode(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    format!(
        "{:02}:{:02}:{:02}",
        total / 3600,
        (total % 3600) / 60,
        total % 60
    )
}

/// Highest resolution MP4 can keep without losing quality.
///
/// MP4 asks for H.264 + AAC (see `build_download_args`) and YouTube stops
/// publishing H.264 above 1080p, so 1440p/2160p in MP4 silently ends up as
/// 1080p. Above this limit (or asking for "best") MKV keeps the original codec.
pub const MAX_MP4_HEIGHT: u32 = 1080;

/// Container actually used, with the MP4 resolution limit applied.
pub fn effective_container(options: &DownloadOptions) -> Option<&str> {
    let container = options.container.as_deref();
    let too_big_for_mp4 = options.mode == MediaMode::Video
        && options
            .video_height
            .is_none_or(|height| height > MAX_MP4_HEIGHT);
    if container == Some("mp4") && too_big_for_mp4 {
        Some("mkv")
    } else {
        container
    }
}

/// Cadena de selección de formato (`-f`) para video.
fn video_format(height: Option<u32>, container: Option<&str>) -> String {
    let limit = height.map(|h| format!("[height<={h}]")).unwrap_or_default();
    if container == Some("webm") {
        format!("bv*[ext=webm]{limit}+ba[ext=webm]/bv*{limit}+ba/b{limit}")
    } else {
        format!("bv*{limit}+ba/b{limit}")
    }
}

/// Argumentos completos de descarga (comunes + opciones del usuario).
pub fn build_download_args(
    settings: &Settings,
    url: &str,
    options: &DownloadOptions,
) -> Vec<String> {
    let mut args = common_args(settings, url);
    args.extend([
        "--no-playlist".into(),
        "--no-warnings".into(),
        // `--print` implies `--quiet`, which would hide the postprocessor
        // markers (`[Merger]`, `[Metadata]`…) used to show "Processing".
        "--no-quiet".into(),
        "--newline".into(),
        "--progress".into(),
        "--progress-template".into(),
        PROGRESS_TEMPLATE.into(),
        "--print".into(),
        PLAN_TEMPLATE.into(),
        "--print".into(),
        FILE_TEMPLATE.into(),
        "--output".into(),
        "%(title)s [%(id)s].%(ext)s".into(),
    ]);

    // Recorte de un fragmento.
    if let Some(section) = options.clip_section() {
        args.push("--download-sections".into());
        args.push(section);
    }

    // Subtítulos (descarga y, opcionalmente, incrustado).
    if options.subtitles {
        args.push("--write-subs".into());
        if options.subtitle_auto {
            args.push("--write-auto-subs".into());
        }
        args.push("--sub-langs".into());
        args.push(
            options
                .subtitle_langs
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("es")
                .to_string(),
        );

        // Incrustar solo tiene sentido en vídeo; en audio se dejan los .srt.
        if options.embed_subtitles && options.mode == MediaMode::Video {
            args.push("--embed-subs".into());
        } else {
            args.push("--convert-subs".into());
            args.push("srt".into());
        }
    }

    if options.sponsorblock {
        let flag = match options.sponsorblock_mode {
            SponsorBlockMode::Mark => "--sponsorblock-mark",
            SponsorBlockMode::Remove => "--sponsorblock-remove",
        };
        args.push(flag.to_string());
        args.push(options.sponsorblock_categories());
    }

    let filters = options.audio_filters();

    match options.mode {
        MediaMode::Video => {
            let container = effective_container(options);
            args.push("-f".into());
            args.push(video_format(options.video_height, container));

            if container == Some("mp4") {
                // Compatibility first: MP4 only carries H.264 + AAC here.
                args.push("-S".into());
                args.push("vcodec:h264,acodec:aac".into());
            }

            if options.needs_audio_reencode() {
                // Los filtros de audio requieren re-codificar: el merge usa copia de streams.
                args.push("--recode-video".into());
                args.push(container.unwrap_or("mp4").to_string());
                args.push("--postprocessor-args".into());
                args.push(format!("VideoConvertor+ffmpeg:-af {filters}"));
            } else if let Some(container) = container {
                args.push("--merge-output-format".into());
                args.push(container.to_string());
            }
        }
        MediaMode::Audio => {
            args.push("-x".into());

            // Con filtros de audio hay que convertir, no se puede copiar el stream.
            let requested = options.audio_format.as_deref().unwrap_or("best");
            let format = if requested == "best" && !filters.is_empty() {
                "m4a"
            } else {
                requested
            };

            if format != "best" {
                args.push("--audio-format".into());
                args.push(format.to_string());
                if let Some(quality) = options.audio_quality.as_deref() {
                    args.push("--audio-quality".into());
                    args.push(quality.to_string());
                }
            }

            if !filters.is_empty() {
                args.push("--postprocessor-args".into());
                args.push(format!("ExtractAudio+ffmpeg:-af {filters}"));
            }
        }
    }

    if options.embed_metadata {
        args.push("--embed-metadata".into());
    }
    if options.embed_thumbnail {
        args.push("--embed-thumbnail".into());
    }

    args.push(url.to_string());
    args
}

// ---------------------------------------------------------------------------
// Sondeo de metadatos
// ---------------------------------------------------------------------------

/// Tipo de contenido detectado al analizar un enlace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Playlist,
}

/// Un elemento dentro de una playlist.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub title: String,
    /// URL lista para descargar (nunca relativa).
    pub url: String,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProbeInfo {
    pub kind: MediaKind,
    pub title: String,
    pub channel: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub extractor: Option<String>,
    pub formats_count: usize,
    /// Resoluciones de video disponibles, de mayor a menor (vacío en playlists).
    pub heights: Vec<u32>,
    /// Elementos de la playlist (vacío si es un video suelto).
    pub entries: Vec<PlaylistEntry>,
    /// `true` si la lista se recortó (p. ej. una radio de YouTube sin fin).
    pub truncated: bool,
}

/// Traduce errores de yt-dlp a mensajes accionables para el usuario.
pub(crate) fn friendly_error(stderr: &str) -> String {
    let raw = last_error_line(stderr).unwrap_or_else(|| "La operación falló.".to_string());
    let lower = raw.to_lowercase();
    // La detección mira todo el stderr: si yt-dlp termina con un traceback, la
    // última línea no siempre es el mensaje de error.
    let all = stderr.to_lowercase();

    if lower.contains("sign in to confirm") || lower.contains("page needs to be reloaded") {
        return format!(
            "YouTube pidió verificación anti-bot. Abre Ajustes y activa «cookies desde el navegador» (con sesión iniciada en YouTube).\n\nDetalle: {raw}"
        );
    }
    if lower.contains("n challenge") || lower.contains("ejs") {
        return format!(
            "No se pudieron resolver los desafíos JS de YouTube. Instala Node.js o Deno, o revisa tu conexión.\n\nDetalle: {raw}"
        );
    }
    // En Windows, el navegador bloquea su base de cookies mientras está abierto
    // y yt-dlp no puede copiarla (yt-dlp#7271). Es el error más común al usar
    // «cookies desde el navegador» y siempre se ve en Windows.
    if all.contains("could not copy") {
        return format!(
            "No se pudieron leer las cookies porque el navegador tiene bloqueado su archivo de cookies (habitual en Windows, mientras el navegador está abierto).\n\
             Cierra el navegador por completo —también de la bandeja y de segundo plano— y vuelve a probar.\n\
             Alternativa: exporta las cookies a un archivo cookies.txt y usa «Cookies → Desde archivo».\n\n\
             Detalle: {raw}"
        );
    }
    if all.contains("failed to decrypt") {
        return format!(
            "El navegador cifró sus cookies y no se pudieron descifrar. Exporta las cookies a un archivo cookies.txt y usa «Cookies → Desde archivo».\n\nDetalle: {raw}"
        );
    }
    raw
}

/// Consulta los metadatos de un enlace sin descargar nada.
/// Usa `--flat-playlist` para listar playlists enteras muy rápido.
pub async fn probe(settings: &Settings, url: &str) -> Result<ProbeInfo, String> {
    let args = probe_args(settings, url);

    let output = tokio::time::timeout(
        Duration::from_secs(120),
        engine::command(ytdlp_bin())
            .args(&args)
            .stdin(Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| "El sitio no respondió a tiempo. Intenta de nuevo.".to_string())?
    .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;

    if !output.status.success() {
        return Err(friendly_error(&String::from_utf8_lossy(&output.stderr)));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| "yt-dlp devolvió una respuesta inválida.".to_string())?;

    let entries = playlist_entries(&json, url);
    let radio = is_youtube_radio(url);

    Ok(ProbeInfo {
        kind: if entries.is_empty() {
            MediaKind::Video
        } else {
            MediaKind::Playlist
        },
        title: json
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Sin título")
            .to_string(),
        channel: json
            .get("channel")
            .or_else(|| json.get("uploader"))
            .and_then(|v| v.as_str())
            .map(String::from),
        duration: json.get("duration").and_then(|v| v.as_f64()),
        thumbnail: thumbnail_url(&json),
        webpage_url: json
            .get("webpage_url")
            .and_then(|v| v.as_str())
            .unwrap_or(url)
            .to_string(),
        extractor: json
            .get("extractor_key")
            .or_else(|| json.get("extractor"))
            .and_then(|v| v.as_str())
            .map(String::from),
        formats_count: json
            .get("formats")
            .and_then(|v| v.as_array())
            .map(|f| f.len())
            .unwrap_or(0),
        heights: available_heights(&json),
        truncated: radio && entries.len() >= RADIO_PLAYLIST_LIMIT,
        entries,
    })
}

/// Argumentos del sondeo. Limita las radios de YouTube, que no tienen fin.
fn probe_args(settings: &Settings, url: &str) -> Vec<String> {
    let mut args = common_args(settings, url);
    args.extend([
        "--skip-download".into(),
        "--flat-playlist".into(),
        "--no-warnings".into(),
    ]);

    if is_youtube_radio(url) {
        args.push("--playlist-end".into());
        args.push(RADIO_PLAYLIST_LIMIT.to_string());
    }

    args.push("--dump-single-json".into());
    args.push(url.to_string());
    args
}

/// Mejor miniatura disponible en un objeto de yt-dlp.
fn thumbnail_url(json: &serde_json::Value) -> Option<String> {
    if let Some(url) = json.get("thumbnail").and_then(|v| v.as_str()) {
        return Some(url.to_string());
    }
    json.get("thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|list| list.last())
        .and_then(|thumb| thumb.get("url"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Elementos de una playlist, si el JSON describe una.
/// Se eliminan los repetidos por `id`: las radios/mixes de YouTube repiten
/// vídeos y con claves duplicadas el listado del frontend se rompe.
fn playlist_entries(json: &serde_json::Value, parent_url: &str) -> Vec<PlaylistEntry> {
    let is_playlist = json.get("_type").and_then(|v| v.as_str()) == Some("playlist")
        || json.get("entries").is_some_and(|v| v.is_array());
    if !is_playlist {
        return Vec::new();
    }

    let mut seen = std::collections::HashSet::new();
    json.get("entries")
        .and_then(|v| v.as_array())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| playlist_entry(entry, parent_url))
                .filter(|entry| entry.id.is_empty() || seen.insert(entry.id.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn playlist_entry(entry: &serde_json::Value, parent_url: &str) -> Option<PlaylistEntry> {
    if entry.is_null() {
        return None;
    }
    let id = entry
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let url = entry_url(entry, parent_url, &id)?;
    Some(PlaylistEntry {
        id,
        title: entry
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Sin título")
            .to_string(),
        url,
        duration: entry.get("duration").and_then(|v| v.as_f64()),
        thumbnail: thumbnail_url(entry),
    })
}

/// Resuelve la URL de descarga de una entrada de playlist. Muchos extractores,
/// con `--flat-playlist`, solo devuelven el id del video.
fn entry_url(entry: &serde_json::Value, parent_url: &str, id: &str) -> Option<String> {
    for key in ["webpage_url", "original_url", "url"] {
        if let Some(value) = entry.get(key).and_then(|v| v.as_str())
            && value.starts_with("http")
        {
            return Some(value.to_string());
        }
    }
    if !id.is_empty() && is_youtube(parent_url) {
        return Some(format!("https://www.youtube.com/watch?v={id}"));
    }
    None
}

/// Resoluciones de video presentes en la respuesta de yt-dlp.
fn available_heights(json: &serde_json::Value) -> Vec<u32> {
    let mut heights: Vec<u32> = json
        .get("formats")
        .and_then(|v| v.as_array())
        .map(|formats| {
            formats
                .iter()
                .filter_map(|f| f.get("height").and_then(|h| h.as_u64()))
                .filter(|h| *h > 0)
                .map(|h| h as u32)
                .collect()
        })
        .unwrap_or_default();
    heights.sort_unstable_by(|a, b| b.cmp(a));
    heights.dedup();
    heights.truncate(8);
    heights
}

// ---------------------------------------------------------------------------
// Proceso de descarga
// ---------------------------------------------------------------------------

/// Which stream a progress line belongs to. yt-dlp downloads the video and the
/// audio as separate files and counts each one from zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamKind {
    Video,
    Audio,
    #[default]
    Unknown,
}

impl StreamKind {
    /// Audio-only formats report `vcodec = none`; `NA`/empty mean "no idea".
    fn from_vcodec(raw: &str) -> Self {
        match raw.trim() {
            "none" => Self::Audio,
            "" | "NA" => Self::Unknown,
            _ => Self::Video,
        }
    }
}

/// Un tick de progreso del protocolo `__DL__`.
#[derive(Debug, Clone, PartialEq)]
pub struct ProgressTick {
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed_bps: Option<f64>,
    pub eta_secs: Option<u64>,
    pub percent: Option<f64>,
    pub stream: StreamKind,
}

/// Evento interpretado de una línea de la salida de yt-dlp.
#[derive(Debug, Clone, PartialEq)]
pub enum LineEvent {
    Progress(ProgressTick),
    /// Total size of the whole job, before it starts (video + audio).
    Plan(u64),
    File(String),
    Phase(String),
}

/// Adds up the streams of one download. yt-dlp fetches the video and the audio
/// as two separate files and restarts the counter for each one, which used to
/// make the bar jump from 100% back to 0% halfway through.
#[derive(Debug, Default)]
pub struct ProgressMeter {
    /// Bytes of the streams already finished (everything before this stream).
    banked_bytes: u64,
    last_downloaded: u64,
    last_total: Option<u64>,
    /// Streams already seen (`> 0` means there was at least one before).
    streams: u32,
    kind: StreamKind,
    /// Size of the whole job, when yt-dlp announced it up front.
    planned: Option<u64>,
}

impl ProgressMeter {
    /// Total size of the job (video + audio), announced before downloading.
    pub fn plan(&mut self, total: u64) {
        self.planned = Some(total);
    }

    /// Feeds one tick and returns it with the totals of the whole job.
    pub fn feed(&mut self, tick: ProgressTick) -> ProgressTick {
        let downloaded = tick.downloaded_bytes.unwrap_or(0);
        if downloaded < self.last_downloaded {
            // The counter went back to zero: the previous stream is done.
            self.banked_bytes += self.last_total.unwrap_or(self.last_downloaded);
            self.streams += 1;
        }
        self.last_downloaded = downloaded;
        self.last_total = tick.total_bytes;
        self.kind = tick.stream;

        let all = self
            .planned
            .or(tick.total_bytes.map(|stream| self.banked_bytes + stream));
        let done = self.banked_bytes + downloaded;
        let done = all.map_or(done, |all| done.min(all));
        let percent = match all {
            Some(all) if all > 0 => Some((done as f64 / all as f64 * 100.0).min(100.0)),
            _ => None,
        };
        let eta_secs = match (all, tick.speed_bps) {
            (Some(all), Some(speed)) if speed > 0.0 && all > done => {
                Some(((all - done) as f64 / speed) as u64)
            }
            _ => tick.eta_secs,
        };

        ProgressTick {
            downloaded_bytes: Some(done),
            total_bytes: all,
            speed_bps: tick.speed_bps,
            eta_secs,
            percent,
            stream: tick.stream,
        }
    }

    /// Stream being downloaded, for the row label. The ordinal is a fallback
    /// when yt-dlp does not report the codec: the audio comes second.
    pub fn stream_label(&self) -> Option<&'static str> {
        match self.kind {
            StreamKind::Video => Some("video"),
            StreamKind::Audio => Some("audio"),
            StreamKind::Unknown if self.streams > 0 => Some("audio"),
            StreamKind::Unknown => None,
        }
    }
}

/// Postprocesadores cuyo inicio se muestra como «Procesando» en la cola.
const PHASE_MARKERS: [&str; 9] = [
    "[ExtractAudio]",
    "[Merger]",
    "[VideoConvertor]",
    "[VideoRemuxer]",
    "[Metadata]",
    "[EmbedThumbnail]",
    "[FixupM3u8]",
    "[FixupStretched]",
    "[MoveFiles]",
];

/// Interpreta una línea de stdout de yt-dlp.
pub fn parse_line(line: &str) -> Option<LineEvent> {
    if let Some(rest) = line.strip_prefix("__DL__") {
        let parts: Vec<&str> = rest.split('|').collect();
        if parts.len() >= 5 {
            return Some(LineEvent::Progress(progress_tick(&parts)));
        }
        return None;
    }
    if let Some(path) = line.strip_prefix("__FILE__") {
        return Some(LineEvent::File(path.trim().to_string()));
    }
    if let Some(raw) = line.strip_prefix("__PLAN__")
        && let Some(total) = opt_f64(raw).map(|v| v as u64)
        && total > 0
    {
        return Some(LineEvent::Plan(total));
    }
    if let Some(marker) = PHASE_MARKERS.iter().find(|m| line.starts_with(**m)) {
        let name = marker.trim_start_matches('[').trim_end_matches(']');
        return Some(LineEvent::Phase(name.to_string()));
    }
    None
}

/// Construye un `ProgressTick` a partir de los campos del template.
/// El porcentaje y el ETA se calculan aquí: `progress.percent` de yt-dlp
/// devuelve `NA` en muchas versiones.
fn progress_tick(parts: &[&str]) -> ProgressTick {
    let downloaded = opt_f64(parts[0]).map(|v| v as u64);
    let total = opt_f64(parts[1])
        .map(|v| v as u64)
        .or_else(|| opt_f64(parts[2]).map(|v| v as u64));
    let speed_bps = opt_f64(parts[3]);

    let percent = match (downloaded, total) {
        (Some(done), Some(all)) if all > 0 => Some((done as f64 / all as f64 * 100.0).min(100.0)),
        _ => None,
    };

    let eta_secs =
        opt_f64(parts[4])
            .map(|v| v as u64)
            .or_else(|| match (downloaded, total, speed_bps) {
                (Some(done), Some(all), Some(speed)) if speed > 0.0 && all > done => {
                    Some(((all - done) as f64 / speed) as u64)
                }
                _ => None,
            });

    let stream = parts
        .get(5)
        .map(|s| StreamKind::from_vcodec(s))
        .unwrap_or_default();

    ProgressTick {
        downloaded_bytes: downloaded,
        total_bytes: total,
        speed_bps,
        eta_secs,
        percent,
        stream,
    }
}

/// Lanza el proceso de yt-dlp con las tuberías listas para leer.
pub fn spawn_process(
    settings: &Settings,
    url: &str,
    options: &DownloadOptions,
    out_dir: &Path,
) -> Result<(Child, ChildStdout, ChildStderr), String> {
    let args = build_download_args(settings, url, options);

    let mut child = engine::command(ytdlp_bin())
        .args(&args)
        .current_dir(out_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("No se pudo ejecutar yt-dlp: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "No se pudo leer la salida de yt-dlp.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "No se pudo leer el error de yt-dlp.".to_string())?;

    Ok((child, stdout, stderr))
}

/// Versión que reporta el yt-dlp resuelto (`--version`), si se puede ejecutar.
pub async fn ytdlp_version() -> Option<String> {
    let (path, _) = engine::ytdlp();
    version_of(&path, "--version").await
}

/// Información del motor para mostrarla en Ajustes.
pub async fn engine_info() -> EngineInfo {
    let ytdlp = engine::resolve("yt-dlp", "DOWNLINK_YTDLP");
    let ytdlp_version = match ytdlp.as_ref() {
        Some((path, _)) => version_of(path, "--version").await,
        None => None,
    };

    let ffmpeg = engine::ffmpeg();
    let ffmpeg_version = match ffmpeg.as_ref() {
        Some((path, _)) => ffmpeg_version_of(path).await,
        None => None,
    };

    let js_runtime = resolve_js_runtime();
    EngineInfo {
        ytdlp_version,
        ytdlp_source: ytdlp.as_ref().map(|(_, source)| source.label().to_string()),
        ytdlp_path: ytdlp.as_ref().map(|(path, _)| path.display().to_string()),
        js_runtime: js_runtime.as_ref().map(|runtime| runtime.name.clone()),
        js_runtime_source: js_runtime
            .as_ref()
            .map(|runtime| runtime.source.label().to_string()),
        ffmpeg_path: ffmpeg.as_ref().map(|(path, _)| path.display().to_string()),
        ffmpeg_version,
        ffmpeg_source: ffmpeg
            .as_ref()
            .map(|(_, source)| source.label().to_string()),
    }
}

/// Ejecuta un binario con un argumento de versión y devuelve su salida limpia.
async fn version_of(path: &Path, arg: &str) -> Option<String> {
    engine::command(path)
        .arg(arg)
        .stdin(Stdio::null())
        .output()
        .await
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Versión del ffmpeg resuelto (`-version`), si está instalado y se puede ejecutar.
pub async fn ffmpeg_version() -> Option<String> {
    let (path, _) = engine::ffmpeg()?;
    ffmpeg_version_of(&path).await
}

async fn ffmpeg_version_of(path: &Path) -> Option<String> {
    let output = engine::command(path)
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .await
        .ok()?;
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .to_string();
    // "ffmpeg version 8.1.3 Copyright..." → "8.1.3"
    first_line.split_whitespace().nth(2).map(|v| v.to_string())
}

fn last_error_line(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|line| !line.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with(mode: CookiesMode, browser: Option<&str>, file: Option<&str>) -> Settings {
        Settings {
            cookies_mode: mode,
            cookies_browser: browser.map(String::from),
            cookies_file: file.map(String::from),
            ..Settings::default()
        }
    }

    fn no_cookies() -> Settings {
        Settings::default()
    }

    #[test]
    fn incluye_cookies_del_navegador() {
        let settings = settings_with(CookiesMode::Browser, Some("brave"), None);
        let args = common_args(&settings, "https://example.com/video");
        assert!(
            args.windows(2)
                .any(|w| w == ["--cookies-from-browser", "brave"])
        );
    }

    #[test]
    fn incluye_archivo_de_cookies() {
        let settings = settings_with(CookiesMode::File, None, Some("/tmp/cookies.txt"));
        let args = common_args(&settings, "https://example.com/video");
        assert!(
            args.windows(2)
                .any(|w| w == ["--cookies", "/tmp/cookies.txt"])
        );
    }

    #[test]
    fn sin_cookies_no_agrega_flags() {
        let settings = settings_with(CookiesMode::None, Some("brave"), None);
        let args = common_args(&settings, "https://example.com/video");
        assert!(!args.iter().any(|a| a == "--cookies-from-browser"));
        assert!(!args.iter().any(|a| a == "--cookies"));
    }

    #[test]
    fn agrega_ejs_solo_para_youtube() {
        let settings = Settings::default();
        let youtube = common_args(&settings, "https://youtu.be/abc");
        let otro = common_args(&settings, "https://vimeo.com/123");
        assert!(youtube.iter().any(|a| a == "ejs:github"));
        assert!(!otro.iter().any(|a| a == "ejs:github"));
    }

    #[test]
    fn detecta_dominios_de_youtube() {
        assert!(is_youtube("https://www.youtube.com/watch?v=x"));
        assert!(is_youtube("https://YOUTU.BE/x"));
        assert!(!is_youtube("https://vimeo.com/1"));
    }

    /// yt-dlp reconoce el runtime como `quickjs`, pero el binario se llama `qjs`.
    /// Antes la lista usaba `qjs` como nombre y yt-dlp lo ignoraba.
    #[test]
    fn el_runtime_quickjs_usa_el_nombre_que_yt_dlp_reconoce() {
        assert!(JS_RUNTIMES.contains(&("quickjs", "qjs")));
        assert_eq!(MANAGED_JS_RUNTIME_BIN, "qjs");
    }

    #[test]
    fn detecta_radios_de_youtube() {
        assert!(is_youtube_radio(
            "https://www.youtube.com/watch?v=4iSvoQNfrrk&list=RD4iSvoQNfrrk&start_radio=1"
        ));
        assert!(is_youtube_radio(
            "https://www.youtube.com/playlist?list=RDCLAK5uy_abc"
        ));
        assert!(!is_youtube_radio(
            "https://www.youtube.com/playlist?list=PLabc"
        ));
        assert!(!is_youtube_radio("https://vimeo.com/album/1/start_radio=1"));
    }

    #[test]
    fn limita_las_radios_al_sondear() {
        let radio = probe_args(
            &no_cookies(),
            "https://www.youtube.com/watch?v=x&list=RDx&start_radio=1",
        );
        assert!(
            radio.windows(2).any(|w| w == ["--playlist-end", "50"]),
            "la radio debería limitar el número de elementos"
        );

        let playlist = probe_args(&no_cookies(), "https://www.youtube.com/playlist?list=PLabc");
        assert!(!playlist.iter().any(|a| a == "--playlist-end"));
        assert_eq!(
            playlist.last().unwrap(),
            "https://www.youtube.com/playlist?list=PLabc"
        );
    }

    #[test]
    fn elimina_entradas_repetidas_de_la_playlist() {
        let json = serde_json::json!({
            "_type": "playlist",
            "entries": [
                { "id": "aaa", "title": "Primera" },
                { "id": "bbb", "title": "Segunda" },
                { "id": "aaa", "title": "Primera (repetida)" },
                { "id": "ccc", "title": "Tercera" }
            ]
        });
        let entries = playlist_entries(&json, "https://www.youtube.com/playlist?list=RD1");
        assert_eq!(
            entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["aaa", "bbb", "ccc"]
        );
        assert_eq!(entries[0].title, "Primera");
    }

    #[test]
    fn errores_anti_bot_sugieren_ajustes() {
        let msg = friendly_error(
            "WARNING: algo\nERROR: [youtube] x: Sign in to confirm you're not a bot. Use --cookies-from-browser",
        );
        assert!(msg.contains("Ajustes"));
        assert!(msg.contains("Sign in to confirm"));
    }

    /// En Windows el navegador bloquea su base de cookies y yt-dlp no puede
    /// copiarla (yt-dlp#7271). El mensaje debe decir qué hacer.
    #[test]
    fn error_de_cookies_bloqueadas_explica_que_cierre_el_navegador() {
        let stderr = "Extracting cookies from brave\n\
                      [debug] Extracting cookies from: \"C:\\Users\\x\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\Network\\Cookies\"\n\
                      ERROR: Could not copy Chrome cookie database. See https://github.com/yt-dlp/yt-dlp/issues/7271";
        let msg = friendly_error(stderr);
        assert!(msg.contains("Cierra el navegador por completo"), "{msg}");
        assert!(msg.contains("cookies.txt"), "{msg}");
        assert!(msg.contains("Detalle:"), "{msg}");
        assert!(
            msg.contains("Could not copy Chrome cookie database"),
            "{msg}"
        );
    }

    /// La detección mira todo el stderr, no solo la última línea: con pip o
    /// verbose, yt-dlp puede terminar en un traceback.
    #[test]
    fn error_de_cookies_bloqueadas_se_detecta_tambien_con_traceback() {
        let stderr = "ERROR: Could not copy Chrome cookie database. See https://github.com/yt-dlp/yt-dlp/issues/7271\n\
                      Traceback (most recent call last):\n\
                      File \"yt_dlp\\cookies.py\", line 365, in _extract_chrome_cookies\n\
                      raise DownloadError(message)  # force exit";
        let msg = friendly_error(stderr);
        assert!(msg.contains("Cierra el navegador por completo"), "{msg}");
    }

    /// Cookies cifradas por el navegador: otro problema distinto (no es bloqueo).
    #[test]
    fn error_de_cookies_sin_descifrar_sugiere_cookies_txt() {
        let msg = friendly_error("ERROR: [youtube] x: failed to decrypt chrome cookies");
        assert!(msg.contains("cookies.txt"), "{msg}");
        assert!(msg.contains("descifrar"), "{msg}");
    }

    #[test]
    fn lee_numeros_con_valores_na() {
        assert_eq!(opt_f64("NA"), None);
        assert_eq!(opt_f64(""), None);
        assert_eq!(opt_f64("12.5"), Some(12.5));
        assert_eq!(opt_f64("100"), Some(100.0));
    }

    #[test]
    fn video_1080p_mp4_con_metadatos() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            video_height: Some(1080),
            container: Some("mp4".into()),
            embed_metadata: true,
            embed_thumbnail: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(
            args.windows(2)
                .any(|w| w == ["-f", "bv*[height<=1080]+ba/b[height<=1080]"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["-S", "vcodec:h264,acodec:aac"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--merge-output-format", "mp4"])
        );
        assert!(args.iter().any(|a| a == "--embed-metadata"));
        assert!(args.iter().any(|a| a == "--embed-thumbnail"));
        assert!(!args.iter().any(|a| a == "--recode-video"));
    }

    #[test]
    fn mp4_por_encima_de_1080p_pasa_a_mkv() {
        // 1440p+ (o "mejor") en MP4 se quedaría en 1080p por culpa de H.264.
        for height in [Some(1440), Some(2160), None] {
            let options = DownloadOptions {
                mode: MediaMode::Video,
                video_height: height,
                container: Some("mp4".into()),
                ..Default::default()
            };
            let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
            assert!(
                args.windows(2)
                    .any(|w| w == ["--merge-output-format", "mkv"]),
                "altura {height:?}: {args:?}"
            );
            assert!(!args.iter().any(|a| a == "-S"), "altura {height:?}");
        }
    }

    #[test]
    fn mkv_y_webm_ignoran_el_limite_de_mp4() {
        let en_4k = |container: &str| DownloadOptions {
            mode: MediaMode::Video,
            video_height: Some(2160),
            container: Some(container.into()),
            ..Default::default()
        };

        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &en_4k("mkv"));
        assert!(
            args.windows(2)
                .any(|w| w == ["--merge-output-format", "mkv"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["-f", "bv*[height<=2160]+ba/b[height<=2160]"])
        );

        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &en_4k("webm"));
        assert!(
            args.windows(2)
                .any(|w| w == ["--merge-output-format", "webm"])
        );
    }

    #[test]
    fn el_limite_de_mp4_no_toca_el_audio() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            video_height: Some(2160),
            container: Some("mp4".into()),
            audio_format: Some("mp3".into()),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.iter().any(|a| a == "-x"));
        assert!(!args.iter().any(|a| a == "--merge-output-format"));
    }

    #[test]
    fn video_webm_prefiere_webm() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            container: Some("webm".into()),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(
            args.iter()
                .any(|a| a == "bv*[ext=webm]+ba[ext=webm]/bv*+ba/b")
        );
        assert!(!args.iter().any(|a| a == "-S"));
    }

    #[test]
    fn mp3_con_volumen_y_normalizacion() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("mp3".into()),
            audio_quality: Some("320K".into()),
            volume_db: Some(6.0),
            normalize_audio: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(args.contains(&"-x".to_string()));
        assert!(args.windows(2).any(|w| w == ["--audio-format", "mp3"]));
        assert!(args.windows(2).any(|w| w == ["--audio-quality", "320K"]));
        let ppa = args
            .windows(2)
            .find(|w| w[0] == "--postprocessor-args")
            .expect("falta --postprocessor-args");
        assert!(ppa[1].starts_with("ExtractAudio+ffmpeg:-af volume=6dB,loudnorm"));
    }

    #[test]
    fn audio_original_con_volumen_fuerza_conversion() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("best".into()),
            volume_db: Some(-4.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.windows(2).any(|w| w == ["--audio-format", "m4a"]));
    }

    #[test]
    fn audio_original_sin_filtros_no_convierte() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("best".into()),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a == "--audio-format"));
        assert!(args.contains(&"-x".to_string()));
    }

    #[test]
    fn volumen_cero_no_genera_filtro() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("flac".into()),
            volume_db: Some(0.0),
            ..Default::default()
        };
        assert!(options.audio_filters().is_empty());
        assert!(!options.needs_audio_reencode());
    }

    #[test]
    fn video_con_volumen_re_codifica() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            video_height: Some(720),
            container: Some("mp4".into()),
            volume_db: Some(3.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);

        assert!(args.windows(2).any(|w| w == ["--recode-video", "mp4"]));
        assert!(
            args.iter()
                .any(|a| a.starts_with("VideoConvertor+ffmpeg:-af volume=3"))
        );
        assert!(!args.iter().any(|a| a == "--merge-output-format"));
    }

    #[test]
    fn la_url_va_al_final() {
        let options = DownloadOptions::default();
        let args = build_download_args(&no_cookies(), "https://vimeo.com/999", &options);
        assert_eq!(args.last().unwrap(), "https://vimeo.com/999");
    }

    #[test]
    fn recorta_con_secciones() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            clip_start: Some(65.0),
            clip_end: Some(125.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(
            args.windows(2)
                .any(|w| w == ["--download-sections", "*00:01:05-00:02:05"])
        );
        // No forzamos keyframes: re-codificaría con los códecs por defecto de ffmpeg
        // y rompería la preferencia H.264/AAC del MP4.
        assert!(!args.iter().any(|a| a == "--force-keyframes-at-cuts"));
    }

    #[test]
    fn recorte_sin_fin_llega_hasta_el_final() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            clip_start: Some(30.0),
            clip_end: None,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(
            args.iter().any(|a| a == "*00:00:30-inf"),
            "esperaba el recorte abierto, args: {args:?}"
        );
    }

    #[test]
    fn recorte_en_audio_tambien_usa_secciones() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: Some("mp3".into()),
            clip_start: Some(10.0),
            clip_end: Some(20.0),
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(
            args.windows(2)
                .any(|w| w == ["--download-sections", "*00:00:10-00:00:20"])
        );
    }

    #[test]
    fn sponsorblock_anade_remove_con_categorias() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            sponsorblock: true,
            sponsorblock_mode: SponsorBlockMode::Remove,
            sponsorblock_categories: vec!["sponsor".to_string(), "intro".to_string()],
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.iter().any(|a| a == "--sponsorblock-remove"));
        assert!(args.iter().any(|a| a == "sponsor,intro"));
    }

    #[test]
    fn sponsorblock_usa_defecto_si_vacio() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            sponsorblock: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.iter().any(|a| a == "sponsor"));
    }

    #[test]
    fn sponsorblock_mark_anade_flag() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            sponsorblock: true,
            sponsorblock_mode: SponsorBlockMode::Mark,
            sponsorblock_categories: vec!["interaction".to_string()],
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.iter().any(|a| a == "--sponsorblock-mark"));
    }

    #[test]
    fn sponsorblock_sin_activo_no_anade_flags() {
        let options = DownloadOptions::default();
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a.contains("sponsorblock")));
    }

    #[test]
    fn sin_recorte_no_pasa_download_sections() {
        let options = DownloadOptions::default();
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a == "--download-sections"));
    }

    #[test]
    fn subtitulos_incrustados_en_video() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            subtitles: true,
            subtitle_langs: Some("es,en".into()),
            subtitle_auto: true,
            embed_subtitles: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(args.iter().any(|a| a == "--write-subs"));
        assert!(args.iter().any(|a| a == "--write-auto-subs"));
        assert!(args.windows(2).any(|w| w == ["--sub-langs", "es,en"]));
        assert!(args.iter().any(|a| a == "--embed-subs"));
        assert!(!args.iter().any(|a| a == "--convert-subs"));
    }

    #[test]
    fn subtitulos_como_archivos_srt() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            subtitles: true,
            subtitle_langs: None,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        // Idioma por defecto cuando no se indica.
        assert!(args.windows(2).any(|w| w == ["--sub-langs", "es"]));
        assert!(!args.iter().any(|a| a == "--embed-subs"));
        assert!(args.windows(2).any(|w| w == ["--convert-subs", "srt"]));
    }

    #[test]
    fn subtitulos_en_audio_no_se_incrustan() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            subtitles: true,
            embed_subtitles: true,
            ..Default::default()
        };
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a == "--embed-subs"));
        assert!(args.windows(2).any(|w| w == ["--convert-subs", "srt"]));
    }

    #[test]
    fn sin_subtitulos_no_agrega_flags() {
        let options = DownloadOptions::default();
        let args = build_download_args(&no_cookies(), "https://vimeo.com/1", &options);
        assert!(!args.iter().any(|a| a == "--write-subs"));
        assert!(!args.iter().any(|a| a == "--sub-langs"));
        assert!(!args.iter().any(|a| a == "--embed-subs"));
    }

    #[test]
    fn formatea_timecodes() {
        assert_eq!(timecode(0.0), "00:00:00");
        assert_eq!(timecode(65.0), "00:01:05");
        assert_eq!(timecode(3725.0), "01:02:05");
        // Redondea al segundo más cercano.
        assert_eq!(timecode(4.6), "00:00:05");
    }

    #[test]
    fn resoluciones_disponibles_ordenadas_y_sin_repetir() {
        let json = serde_json::json!({
            "formats": [
                {"height": 720}, {"height": 1440}, {"height": 720},
                {"height": null}, {"height": 0}, {"height": 1080}
            ]
        });
        assert_eq!(available_heights(&json), vec![1440, 1080, 720]);
    }

    #[test]
    fn detecta_playlist_y_sus_entradas() {
        let json = serde_json::json!({
            "_type": "playlist",
            "title": "Mi playlist",
            "webpage_url": "https://www.youtube.com/playlist?list=PL1",
            "entries": [
                { "id": "aaa", "title": "Primera", "duration": 60.0 },
                { "id": "bbb", "title": "Segunda", "duration": null },
                null
            ]
        });
        let entries = playlist_entries(&json, "https://www.youtube.com/playlist?list=PL1");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].title, "Primera");
        assert_eq!(entries[0].url, "https://www.youtube.com/watch?v=aaa");
        assert_eq!(entries[0].duration, Some(60.0));
        assert!(entries[1].duration.is_none());
    }

    #[test]
    fn video_suelto_no_tiene_entradas() {
        let json = serde_json::json!({
            "title": "Un video",
            "webpage_url": "https://vimeo.com/123",
            "formats": [{ "height": 720 }]
        });
        assert!(playlist_entries(&json, "https://vimeo.com/123").is_empty());
    }

    #[test]
    fn usa_la_url_completa_de_la_entrada_si_existe() {
        let json = serde_json::json!({
            "_type": "playlist",
            "entries": [{ "id": "x", "title": "Con URL", "url": "https://vimeo.com/999" }]
        });
        let entries = playlist_entries(&json, "https://vimeo.com/album/1");
        assert_eq!(entries[0].url, "https://vimeo.com/999");
    }

    #[test]
    fn toma_la_ultima_miniatura_disponible() {
        let json = serde_json::json!({
            "thumbnails": [
                { "url": "https://x/small.jpg" },
                { "url": "https://x/big.jpg" }
            ]
        });
        assert_eq!(thumbnail_url(&json).as_deref(), Some("https://x/big.jpg"));
    }

    #[test]
    fn sponsorblock_categorias_desconocidas_se_ignoran() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            sponsorblock: true,
            sponsorblock_categories: vec!["sponsor".to_string(), "hack".to_string()],
            ..Default::default()
        };
        assert_eq!(options.sponsorblock_categories(), "sponsor");
    }

    #[test]
    fn sponsorblock_categorias_repetidas_se_quitan() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            sponsorblock: true,
            sponsorblock_categories: vec!["intro".to_string(), "intro".to_string()],
            ..Default::default()
        };
        assert_eq!(options.sponsorblock_categories(), "intro");
    }

    #[test]
    fn sponsorblock_acepta_categorias_con_espacios() {
        let options = DownloadOptions {
            mode: MediaMode::Video,
            sponsorblock: true,
            sponsorblock_categories: vec![" Selfpromo ".to_string()],
            ..Default::default()
        };
        assert_eq!(options.sponsorblock_categories(), "selfpromo");
    }

    #[test]
    fn interpreta_lineas_de_progreso() {
        // downloaded | total | total_estimate | speed | eta | vcodec
        let event = parse_line("__DL__1000|4000|NA|524288.0|12|vp9");
        assert_eq!(
            event,
            Some(LineEvent::Progress(ProgressTick {
                downloaded_bytes: Some(1000),
                total_bytes: Some(4000),
                speed_bps: Some(524288.0),
                eta_secs: Some(12),
                percent: Some(25.0),
                stream: StreamKind::Video,
            }))
        );
    }

    #[test]
    fn calcula_porcentaje_y_eta_si_faltan() {
        // Sin total real pero con estimación, y sin eta (se calcula con la velocidad).
        let event = parse_line("__DL__1000|NA|4000|1000.0|NA|none");
        assert_eq!(
            event,
            Some(LineEvent::Progress(ProgressTick {
                downloaded_bytes: Some(1000),
                total_bytes: Some(4000),
                speed_bps: Some(1000.0),
                eta_secs: Some(3),
                percent: Some(25.0),
                stream: StreamKind::Audio,
            }))
        );
    }

    #[test]
    fn interpreta_lineas_con_valores_vacios() {
        let event = parse_line("__DL__NA|NA|NA|NA|NA");
        assert_eq!(
            event,
            Some(LineEvent::Progress(ProgressTick {
                downloaded_bytes: None,
                total_bytes: None,
                speed_bps: None,
                eta_secs: None,
                percent: None,
                stream: StreamKind::Unknown,
            }))
        );
    }

    #[test]
    fn interpreta_la_ruta_del_archivo() {
        assert_eq!(
            parse_line("__FILE__/home/user/video.mp4"),
            Some(LineEvent::File("/home/user/video.mp4".to_string()))
        );
    }

    fn tick(done: u64, total: Option<u64>, stream: StreamKind) -> ProgressTick {
        ProgressTick {
            downloaded_bytes: Some(done),
            total_bytes: total,
            speed_bps: None,
            eta_secs: None,
            percent: None,
            stream,
        }
    }

    #[test]
    fn interpreta_el_total_anunciado() {
        assert_eq!(parse_line("__PLAN__475961"), Some(LineEvent::Plan(475961)));
        assert_eq!(parse_line("__PLAN__NA"), None);
        assert_eq!(parse_line("__PLAN__0"), None);
    }

    #[test]
    fn el_medidor_suma_los_dos_streams() {
        let mut meter = ProgressMeter::default();

        // Video: 0 → 4000 de 4000.
        assert_eq!(
            meter
                .feed(tick(2000, Some(4000), StreamKind::Video))
                .percent,
            Some(50.0)
        );
        let end = meter.feed(tick(4000, Some(4000), StreamKind::Video));
        assert_eq!(end.percent, Some(100.0));

        // Audio: el contador vuelve a cero, pero lo anterior ya está guardado.
        let start = meter.feed(tick(0, Some(1000), StreamKind::Audio));
        assert_eq!(start.percent, Some(80.0), "no debe volver al 0 %");
        assert_eq!(start.downloaded_bytes, Some(4000));

        let end = meter.feed(tick(1000, Some(1000), StreamKind::Audio));
        assert_eq!(end.percent, Some(100.0));
        assert_eq!(end.total_bytes, Some(5000));
        assert_eq!(end.downloaded_bytes, Some(5000));
    }

    #[test]
    fn el_medidor_usa_el_total_anunciado() {
        let mut meter = ProgressMeter::default();
        meter.plan(5000);

        let mut percents = vec![];
        for (done, total) in [(1000, 4000), (4000, 4000)] {
            percents.push(
                meter
                    .feed(tick(done, Some(total), StreamKind::Video))
                    .percent,
            );
        }
        // El audio empieza y la barra sigue donde iba, sin bajar.
        percents.push(meter.feed(tick(0, Some(1000), StreamKind::Audio)).percent);
        percents.push(
            meter
                .feed(tick(1000, Some(1000), StreamKind::Audio))
                .percent,
        );

        assert_eq!(
            percents,
            vec![Some(20.0), Some(80.0), Some(80.0), Some(100.0)]
        );
    }

    #[test]
    fn el_medidor_etiqueta_el_stream_que_baja() {
        let mut meter = ProgressMeter::default();
        assert_eq!(meter.stream_label(), None);
        meter.feed(tick(100, Some(4000), StreamKind::Video));
        assert_eq!(meter.stream_label(), Some("video"));
        meter.feed(tick(0, Some(1000), StreamKind::Audio));
        assert_eq!(meter.stream_label(), Some("audio"));

        // Sin códec reportado, el segundo stream se supone audio.
        let mut meter = ProgressMeter::default();
        meter.feed(tick(4000, Some(4000), StreamKind::Unknown));
        assert_eq!(meter.stream_label(), None);
        meter.feed(tick(0, Some(1000), StreamKind::Unknown));
        assert_eq!(meter.stream_label(), Some("audio"));
    }

    /// Salida real de yt-dlp (recortada) de una descarga 240p con mezcla a MKV:
    /// plan, video de 223779 B, audio de 252182 B, fases y ruta final.
    const SALIDA_REAL: [&str; 8] = [
        "__PLAN__475961",
        "__DL__1024|223779|NA|NA|NA|av01.0.00M.08",
        "__DL__223779|223779|NA|18856041.0|0|av01.0.00M.08",
        "__DL__1024|252182|NA|NA|NA|none",
        "__DL__252182|252182|NA|60204221.0|NA|none",
        "[Merger] Merging formats into \"v.mkv\"",
        "[Metadata] Adding metadata to \"v.mkv\"",
        "__FILE__/tmp/v.mkv",
    ];

    #[test]
    fn la_salida_real_de_yt_dlp_acaba_en_100() {
        let mut meter = ProgressMeter::default();
        let mut last_percent = 0.0;
        let mut phases = Vec::new();
        let mut file = None;

        for line in SALIDA_REAL {
            match parse_line(line) {
                Some(LineEvent::Plan(total)) => meter.plan(total),
                Some(LineEvent::Progress(raw)) => {
                    let tick = meter.feed(raw);
                    let percent = tick.percent.expect("el plan da el total");
                    assert!(
                        percent >= last_percent,
                        "la barra retrocedió en «{line}»: {percent} < {last_percent}"
                    );
                    last_percent = percent;
                    assert!(tick.total_bytes == Some(475961), "total del trabajo");
                }
                Some(LineEvent::Phase(name)) => phases.push(name),
                Some(LineEvent::File(path)) => file = Some(path),
                None => panic!("línea sin interpretar: {line}"),
            }
        }

        assert_eq!(last_percent, 100.0);
        assert_eq!(phases, vec!["Merger".to_string(), "Metadata".to_string()]);
        assert_eq!(file.as_deref(), Some("/tmp/v.mkv"));
        assert_eq!(meter.stream_label(), Some("audio"));
    }

    #[test]
    fn detecta_fases_de_postprocesado() {
        assert_eq!(
            parse_line("[ExtractAudio] Destination: cancion.mp3"),
            Some(LineEvent::Phase("ExtractAudio".to_string()))
        );
        assert_eq!(
            parse_line("[Merger] Merging formats into \"video.mp4\""),
            Some(LineEvent::Phase("Merger".to_string()))
        );
        // Las líneas normales de descarga no son fases.
        assert_eq!(parse_line("[download] Destination: video.webm"), None);
        assert_eq!(parse_line("[youtube] Extracting URL"), None);
    }
}
