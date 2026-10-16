//! Apariencia: lectura de la imagen de fondo y de la paleta del escritorio.
//!
//! La imagen se devuelve como `data:` URL para no tener que abrir el protocolo
//! de assets de Tauri (y que funcione igual en desarrollo y en producción).
//! La paleta se lee de pywal/wallust, el estándar de facto del *ricing* en
//! Linux, para que la app combine con el resto del escritorio.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

/// Tope de tamaño para la imagen de fondo (evita reventar el IPC).
const MAX_WALLPAPER_BYTES: u64 = 16 * 1024 * 1024;

/// Paleta exportada por pywal/wallust.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemPalette {
    /// Herramienta de la que salió la paleta (`wallust` o `pywal`).
    pub source: String,
    pub background: String,
    pub foreground: String,
    /// Los 16 colores ANSI (`color0`…`color15`).
    pub colors: Vec<String>,
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Codifica en base64 estándar. Se implementa aquí para no sumar dependencias.
pub fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;

        out.push(BASE64[(n >> 18) as usize & 63] as char);
        out.push(BASE64[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(BASE64[(n >> 6) as usize & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(BASE64[n as usize & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Tipo MIME de una imagen por su extensión.
fn mime_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

/// Lee la imagen y la devuelve como `data:` URL.
pub fn read_wallpaper(path: &str) -> Result<String, String> {
    let p = PathBuf::from(path);
    let mime = mime_for(&p).ok_or_else(|| {
        "El archivo no es una imagen compatible (png, jpg, webp, gif, bmp o avif).".to_string()
    })?;

    let meta = std::fs::metadata(&p)
        .map_err(|_| "No se pudo leer la imagen de fondo seleccionada.".to_string())?;
    if !meta.is_file() {
        return Err("La ruta seleccionada no es un archivo.".to_string());
    }
    if meta.len() > MAX_WALLPAPER_BYTES {
        return Err(format!(
            "La imagen es demasiado grande (máximo {} MB).",
            MAX_WALLPAPER_BYTES / 1024 / 1024
        ));
    }

    let bytes = std::fs::read(&p)
        .map_err(|_| "No se pudo leer la imagen de fondo seleccionada.".to_string())?;
    Ok(format!("data:{mime};base64,{}", base64_encode(&bytes)))
}

/// Recorre el JSON y recoge todos los valores de cadena indexados por clave.
///
/// pywal y wallust no emiten exactamente el mismo esquema (a veces `special`,
/// a veces todo dentro de `colors`), así que se busca de forma tolerante.
fn collect_strings(value: &Value, map: &mut HashMap<String, String>) {
    match value {
        Value::Object(obj) => {
            for (key, val) in obj {
                match val {
                    Value::String(s) => {
                        map.entry(key.to_ascii_lowercase())
                            .or_insert_with(|| s.clone());
                    }
                    other => collect_strings(other, map),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_strings(v, map)),
        _ => {}
    }
}

/// Construye la paleta a partir de un `colors.json` ya parseado.
fn palette_from_json(source: &str, json: &Value) -> Option<SystemPalette> {
    let mut map = HashMap::new();
    collect_strings(json, &mut map);

    let background = map.get("background").cloned()?;
    let foreground = map
        .get("foreground")
        .or_else(|| map.get("color7"))
        .or_else(|| map.get("color15"))
        .cloned()?;

    let colors: Vec<String> = (0..16)
        .map(|i| {
            map.get(&format!("color{i}"))
                .cloned()
                .unwrap_or_else(|| background.clone())
        })
        .collect();

    Some(SystemPalette {
        source: source.to_string(),
        background,
        foreground,
        colors,
    })
}

/// Busca la paleta del escritorio en las ubicaciones de pywal y wallust.
pub fn read_system_palette() -> Option<SystemPalette> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".cache"));

    let candidates = [
        ("wallust", cache.join("wallust").join("colors.json")),
        ("pywal", cache.join("wal").join("colors.json")),
    ];

    for (source, path) in candidates {
        let Ok(raw) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if let Some(palette) = palette_from_json(source, &json) {
            return Some(palette);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_codifica_correctamente() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        // Bytes altos (0xFF) para cubrir el último índice del alfabeto.
        assert_eq!(base64_encode(&[0xff, 0xff, 0xff]), "////");
    }

    #[test]
    fn mime_por_extension() {
        assert_eq!(mime_for(Path::new("a.png")), Some("image/png"));
        assert_eq!(mime_for(Path::new("a.JPG")), Some("image/jpeg"));
        assert_eq!(mime_for(Path::new("a.webp")), Some("image/webp"));
        assert_eq!(mime_for(Path::new("a.txt")), None);
        assert_eq!(mime_for(Path::new("sin_extension")), None);
    }

    #[test]
    fn lee_paleta_estilo_pywal() {
        let json: Value = serde_json::from_str(
            r##"{
                "special": { "background": "#111111", "foreground": "#eeeeee" },
                "colors": { "color0": "#000000", "color4": "#0000ff", "color7": "#c0c0c0" }
            }"##,
        )
        .unwrap();

        let palette = palette_from_json("pywal", &json).unwrap();
        assert_eq!(palette.source, "pywal");
        assert_eq!(palette.background, "#111111");
        assert_eq!(palette.foreground, "#eeeeee");
        assert_eq!(palette.colors.len(), 16);
        assert_eq!(palette.colors[4], "#0000ff");
        // Los colores que falten se rellenan con el fondo.
        assert_eq!(palette.colors[10], "#111111");
    }

    #[test]
    fn lee_paleta_con_todo_en_colors() {
        let json: Value = serde_json::from_str(
            r##"{
                "wallpaper": "/home/u/wall.png",
                "colors": {
                    "background": "#1e1e2e",
                    "foreground": "#cdd6f4",
                    "color0": "#45475a",
                    "color5": "#f5c2e7"
                }
            }"##,
        )
        .unwrap();

        let palette = palette_from_json("wallust", &json).unwrap();
        assert_eq!(palette.background, "#1e1e2e");
        assert_eq!(palette.foreground, "#cdd6f4");
        assert_eq!(palette.colors[5], "#f5c2e7");
    }

    #[test]
    fn sin_background_devuelve_none() {
        let json: Value = serde_json::from_str(r##"{"colors":{"color0":"#000"}}"##).unwrap();
        assert!(palette_from_json("pywal", &json).is_none());
    }

    #[test]
    fn read_wallpaper_rechaza_extensiones_no_soportadas() {
        let err = read_wallpaper("/tmp/no-es-imagen.txt").unwrap_err();
        assert!(err.contains("imagen compatible"));
    }

    #[test]
    fn read_wallpaper_lee_un_archivo_real() {
        let dir = std::env::temp_dir();
        let path = dir.join("downlink_wallpaper_test.png");
        std::fs::write(&path, [0x89, b'P', b'N', b'G']).unwrap();

        let url = read_wallpaper(path.to_str().unwrap()).unwrap();
        assert!(url.starts_with("data:image/png;base64,"));
        assert!(url.ends_with("iVBORw=="));

        let _ = std::fs::remove_file(&path);
    }
}
