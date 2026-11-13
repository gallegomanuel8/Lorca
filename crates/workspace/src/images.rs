//! Imágenes de los documentos con formato: se guardan en una carpeta junto al documento
//! (`notas.md` → `notas_imagenes/`) y se enlazan con una ruta relativa, para que el
//! documento y sus imágenes se puedan mover juntos y se vean igual en otros editores.

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::WorkspaceError;

/// Extensiones de imagen que Lorca sabe mostrar.
pub const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// ¿Es `path` una imagen que Lorca sabe mostrar (por la extensión)?
pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// Carpeta de las imágenes de `document`: `<carpeta>/<nombre>_imagenes`.
pub fn images_dir(document: &Path) -> Option<PathBuf> {
    let parent = document.parent()?;
    let stem = document.file_stem()?.to_string_lossy();
    Some(parent.join(format!("{stem}_imagenes")))
}

/// Ruta con la que `document` enlaza `image`: relativa a la carpeta del documento si la
/// imagen está dentro (siempre con `/`), absoluta si no.
pub fn image_target(document: &Path, image: &Path) -> String {
    let relative = document
        .parent()
        .and_then(|parent| image.strip_prefix(parent).ok());
    match relative {
        Some(relative) => relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        None => image.to_string_lossy().replace('\\', "/"),
    }
}

/// Descripción inicial de una imagen: el nombre del archivo sin extensión.
pub fn image_alt(image: &Path) -> String {
    image
        .file_stem()
        .map(|stem| stem.to_string_lossy().replace(['[', ']'], ""))
        .unwrap_or_default()
}

/// Archivo al que apunta `target` en `document` (`None` si es una dirección de internet o
/// si es relativa y el documento no tiene ruta). Entiende `file://` y `%20`.
pub fn resolve_image(document: Option<&Path>, target: &str) -> Option<PathBuf> {
    if target.contains("://") && !target.starts_with("file://") {
        return None;
    }
    let target = percent_decode(target.strip_prefix("file://").unwrap_or(target));
    let path = PathBuf::from(&target);
    if path.is_absolute() {
        return Some(path);
    }
    let parent = document?.parent()?;
    Some(parent.join(path))
}

/// Decodifica las secuencias `%XX` (las demás se dejan tal cual).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while let Some(&byte) = bytes.get(i) {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (byte, hex) {
            (b'%', Some(value)) => {
                out.push(value);
                i += 3;
            }
            _ => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Deja `source` a disposición de `document` y devuelve dónde queda. Si ya está dentro de
/// la carpeta del documento se usa tal cual; si no, se copia a su carpeta de imágenes.
pub fn import_image(document: &Path, source: &Path) -> Result<PathBuf, WorkspaceError> {
    let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let source = canonical(source);
    if let Some(parent) = document.parent()
        && source.starts_with(canonical(parent))
    {
        return Ok(source);
    }
    let dir = prepare_dir(document)?;
    let stem = source
        .file_stem()
        .map_or_else(|| "imagen".to_owned(), |s| s.to_string_lossy().into_owned());
    let ext = source
        .extension()
        .map_or_else(|| "png".to_owned(), |e| e.to_string_lossy().to_lowercase());
    let target = unique_path(&dir, &stem, &ext);
    fs::copy(&source, &target).map_err(|source| WorkspaceError::Write {
        path: target.clone(),
        source,
    })?;
    Ok(target)
}

/// Guarda una imagen RGBA (la del portapapeles) como PNG en la carpeta de imágenes de
/// `document` (`imagen-1.png`, `imagen-2.png`…) y devuelve su ruta.
pub fn save_png(
    document: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<PathBuf, WorkspaceError> {
    let dir = prepare_dir(document)?;
    let target = unique_path(&dir, "imagen", "png");
    let image_error = |message: String| WorkspaceError::Image {
        path: target.clone(),
        message,
    };
    let expected = u64::from(width) * u64::from(height) * 4;
    if u64::try_from(rgba.len()).ok() != Some(expected) {
        return Err(image_error(format!(
            "tamaño inesperado ({} bytes para {width}×{height})",
            rgba.len()
        )));
    }
    let file = fs::File::create(&target).map_err(|source| WorkspaceError::Write {
        path: target.clone(),
        source,
    })?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let result = encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba));
    if let Err(error) = result {
        let _ = fs::remove_file(&target);
        return Err(image_error(error.to_string()));
    }
    Ok(target)
}

/// Crea (si hace falta) la carpeta de imágenes de `document`.
fn prepare_dir(document: &Path) -> Result<PathBuf, WorkspaceError> {
    let dir = images_dir(document).ok_or(WorkspaceError::NoPath)?;
    fs::create_dir_all(&dir).map_err(|source| WorkspaceError::Write {
        path: dir.clone(),
        source,
    })?;
    Ok(dir)
}

/// `<dir>/<stem>.<ext>` o, si existe, `<stem>-2`, `<stem>-3`… (para «imagen», desde
/// `imagen-1`).
fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    // Sin separadores ni nombres especiales: solo el último componente.
    let stem = Path::new(stem)
        .components()
        .rev()
        .find_map(|component| match component {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .unwrap_or_else(|| "imagen".to_owned());
    let numbered = stem == "imagen";
    let first = if numbered { 1 } else { 2 };
    if !numbered {
        let plain = dir.join(format!("{stem}.{ext}"));
        if !plain.exists() {
            return plain;
        }
    }
    (first..)
        .map(|n| dir.join(format!("{stem}-{n}.{ext}")))
        .find(|path| !path.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}.{ext}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_go_next_to_the_document() {
        let doc = Path::new("/docs/notas.md");
        assert_eq!(images_dir(doc), Some(PathBuf::from("/docs/notas_imagenes")));
        assert_eq!(
            image_target(doc, Path::new("/docs/notas_imagenes/gato 1.png")),
            "notas_imagenes/gato 1.png"
        );
        assert_eq!(
            image_target(doc, Path::new("/fotos/gato.png")),
            "/fotos/gato.png"
        );
        assert!(is_image_path(Path::new("a/B.JPG")));
        assert!(!is_image_path(Path::new("a/b.txt")));
        assert_eq!(image_alt(Path::new("/x/mi [foto].png")), "mi foto");
    }

    #[test]
    fn resolves_targets() {
        let doc = Path::new("/docs/notas.md");
        assert_eq!(
            resolve_image(Some(doc), "notas_imagenes/gato%201.png"),
            Some(PathBuf::from("/docs/notas_imagenes/gato 1.png"))
        );
        // «/tmp/a.png» solo es absoluta en Unix (en Windows le falta la unidad).
        #[cfg(unix)]
        assert_eq!(
            resolve_image(None, "file:///tmp/a.png"),
            Some(PathBuf::from("/tmp/a.png"))
        );
        assert_eq!(resolve_image(Some(doc), "https://x.org/a.png"), None);
        assert_eq!(resolve_image(None, "a.png"), None);
        assert_eq!(percent_decode("100%_%zz%41"), "100%_%zzA");
    }

    #[test]
    fn imports_copies_outside_files_and_keeps_inside_ones() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("notas.md");
        fs::write(&doc, "").unwrap();
        let inside = dir.path().join("ya.png");
        fs::write(&inside, b"x").unwrap();
        assert_eq!(
            import_image(&doc, &inside).unwrap(),
            fs::canonicalize(&inside).unwrap()
        );

        let other = tempfile::tempdir().unwrap();
        let outside = other.path().join("gato.png");
        fs::write(&outside, b"miau").unwrap();
        let first = import_image(&doc, &outside).unwrap();
        let second = import_image(&doc, &outside).unwrap();
        assert_eq!(first, dir.path().join("notas_imagenes/gato.png"));
        assert_eq!(second, dir.path().join("notas_imagenes/gato-2.png"));
        assert_eq!(fs::read(second).unwrap(), b"miau");
    }

    #[test]
    fn saves_clipboard_images_as_numbered_pngs() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("notas.md");
        let pixels = [255_u8, 0, 0, 255].repeat(4);
        let first = save_png(&doc, 2, 2, &pixels).unwrap();
        let second = save_png(&doc, 2, 2, &pixels).unwrap();
        assert_eq!(first, dir.path().join("notas_imagenes/imagen-1.png"));
        assert_eq!(second, dir.path().join("notas_imagenes/imagen-2.png"));
        assert!(fs::read(&first).unwrap().starts_with(b"\x89PNG"));
        assert!(save_png(&doc, 3, 3, &pixels).is_err());
    }
}
