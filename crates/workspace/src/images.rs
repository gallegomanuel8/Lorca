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

/// Lado mayor (en píxeles) a partir del cual una imagen se reduce al insertarla.
pub const MAX_IMAGE_SIDE: u32 = 2000;
/// Tamaño de archivo a partir del cual una imagen se intenta reducir al insertarla.
pub const MAX_IMAGE_BYTES: u64 = 1024 * 1024;
/// Calidad JPEG de las fotos reducidas.
const JPEG_QUALITY: u8 = 85;

/// Deja `source` a disposición de `document` y devuelve dónde queda.
///
/// Las imágenes grandes (lado mayor de más de [`MAX_IMAGE_SIDE`] o más de
/// [`MAX_IMAGE_BYTES`]) se guardan reducidas en la carpeta de imágenes: las fotos en JPEG,
/// las PNG (capturas) y las que tienen transparencia en PNG, giradas según su orientación
/// EXIF y sin sus metadatos (ubicación incluida). El original no se toca. Las demás se
/// usan tal cual si ya están en la carpeta del documento o se copian a la de imágenes.
pub fn import_image(document: &Path, source: &Path) -> Result<PathBuf, WorkspaceError> {
    let canonical = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let source = canonical(source);
    let stem = source
        .file_stem()
        .map_or_else(|| "imagen".to_owned(), |s| s.to_string_lossy().into_owned());
    if needs_reduction(&source) {
        let dir = prepare_dir(document)?;
        if let Some(reduced) = reduce(&source, &dir, &stem)? {
            return Ok(reduced);
        }
    }
    if let Some(parent) = document.parent()
        && source.starts_with(canonical(parent))
    {
        return Ok(source);
    }
    let dir = prepare_dir(document)?;
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

/// ¿Conviene reducir `path`? Los GIF no (pueden ser animados).
fn needs_reduction(path: &Path) -> bool {
    let is_gif = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"));
    if is_gif {
        return false;
    }
    let too_heavy = fs::metadata(path).is_ok_and(|meta| meta.len() > MAX_IMAGE_BYTES);
    let too_big = image::image_dimensions(path)
        .is_ok_and(|(width, height)| width.max(height) > MAX_IMAGE_SIDE);
    too_heavy || too_big
}

/// Guarda en `dir` una copia reducida de `source`. `None` si la copia no pesaría menos
/// que el original y tampoco hacía falta achicarla (entonces se usa el original).
fn reduce(source: &Path, dir: &Path, stem: &str) -> Result<Option<PathBuf>, WorkspaceError> {
    let image_error = |message: String| WorkspaceError::Image {
        path: source.to_path_buf(),
        message,
    };
    let reader = image::ImageReader::open(source)
        .and_then(image::ImageReader::with_guessed_format)
        .map_err(|error| image_error(error.to_string()))?;
    let format = reader.format();
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| image_error(error.to_string()))?;
    let orientation = image::ImageDecoder::orientation(&mut decoder)
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut picture =
        image::DynamicImage::from_decoder(decoder).map_err(|e| image_error(e.to_string()))?;
    picture.apply_orientation(orientation);
    let oversized = picture.width().max(picture.height()) > MAX_IMAGE_SIDE;
    let picture = shrink(picture);
    // Capturas (PNG, BMP) y transparencias en PNG para no emborronar ni perder el canal
    // alfa; el resto (fotos) en JPEG.
    let lossless = matches!(
        format,
        Some(image::ImageFormat::Png | image::ImageFormat::Bmp)
    ) || picture.color().has_alpha();
    let target = unique_path(dir, stem, if lossless { "png" } else { "jpg" });
    encode(&picture, &target, lossless)?;
    let original = fs::metadata(source).map_or(u64::MAX, |meta| meta.len());
    let reduced = fs::metadata(&target).map_or(0, |meta| meta.len());
    if !oversized && reduced >= original {
        let _ = fs::remove_file(&target);
        return Ok(None);
    }
    Ok(Some(target))
}

/// Achica `picture` para que su lado mayor no pase de [`MAX_IMAGE_SIDE`].
fn shrink(picture: image::DynamicImage) -> image::DynamicImage {
    if picture.width().max(picture.height()) > MAX_IMAGE_SIDE {
        picture.resize(
            MAX_IMAGE_SIDE,
            MAX_IMAGE_SIDE,
            image::imageops::FilterType::CatmullRom,
        )
    } else {
        picture
    }
}

/// Escribe `picture` en `target` como PNG (`lossless`) o JPEG.
fn encode(
    picture: &image::DynamicImage,
    target: &Path,
    lossless: bool,
) -> Result<(), WorkspaceError> {
    let file = fs::File::create(target).map_err(|source| WorkspaceError::Write {
        path: target.to_path_buf(),
        source,
    })?;
    let mut writer = std::io::BufWriter::new(file);
    let result = if lossless {
        picture.write_to(&mut writer, image::ImageFormat::Png)
    } else {
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, JPEG_QUALITY);
        picture.to_rgb8().write_with_encoder(encoder)
    };
    if let Err(error) = result {
        drop(writer);
        let _ = fs::remove_file(target);
        return Err(WorkspaceError::Image {
            path: target.to_path_buf(),
            message: error.to_string(),
        });
    }
    Ok(())
}

/// Guarda una imagen RGBA (la del portapapeles) como PNG en la carpeta de imágenes de
/// `document` (`imagen-1.png`, `imagen-2.png`…), reducida si su lado mayor pasa de
/// [`MAX_IMAGE_SIDE`], y devuelve su ruta.
pub fn save_png(
    document: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<PathBuf, WorkspaceError> {
    let dir = prepare_dir(document)?;
    let target = unique_path(&dir, "imagen", "png");
    let picture = image::RgbaImage::from_raw(width, height, rgba.to_vec()).ok_or_else(|| {
        WorkspaceError::Image {
            path: target.clone(),
            message: format!(
                "tamaño inesperado ({} bytes para {width}×{height})",
                rgba.len()
            ),
        }
    })?;
    encode(
        &shrink(image::DynamicImage::ImageRgba8(picture)),
        &target,
        true,
    )?;
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

    /// Escribe una imagen de prueba de `width`×`height` con ruido (para que no comprima
    /// casi nada) en `path`.
    fn noisy(path: &Path, width: u32, height: u32) {
        let picture = image::RgbImage::from_fn(width, height, |x, y| {
            let v = (x.wrapping_mul(7919) ^ y.wrapping_mul(104_729)).to_le_bytes();
            image::Rgb([v[0], v[1], v[2]])
        });
        picture.save(path).unwrap();
    }

    #[test]
    fn big_photos_are_reduced_to_jpeg_and_small_ones_copied() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("notas.md");
        let other = tempfile::tempdir().unwrap();

        let photo = other.path().join("foto.jpg");
        noisy(&photo, 2400, 1200);
        let stored = import_image(&doc, &photo).unwrap();
        assert_eq!(stored, dir.path().join("notas_imagenes/foto.jpg"));
        assert_eq!(image::image_dimensions(&stored).unwrap(), (2000, 1000));
        // El original no se toca.
        assert_eq!(image::image_dimensions(&photo).unwrap(), (2400, 1200));

        let capture = other.path().join("captura.png");
        noisy(&capture, 2600, 100);
        let stored = import_image(&doc, &capture).unwrap();
        assert_eq!(stored.extension().unwrap(), "png");
        assert_eq!(image::image_dimensions(&stored).unwrap().0, 2000);

        let small = other.path().join("icono.png");
        noisy(&small, 32, 32);
        let stored = import_image(&doc, &small).unwrap();
        assert_eq!(fs::read(&stored).unwrap(), fs::read(&small).unwrap());

        // Una grande que ya está junto al documento: se reduce en una copia.
        let inside = dir.path().join("grande.jpg");
        noisy(&inside, 3000, 300);
        let stored = import_image(&doc, &inside).unwrap();
        assert_eq!(stored, dir.path().join("notas_imagenes/grande.jpg"));
        assert!(inside.exists());
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
        // Una captura enorme se reduce.
        let big = [10_u8, 20, 30, 255].repeat(3000 * 10);
        let reduced = save_png(&doc, 3000, 10, &big).unwrap();
        assert_eq!(image::image_dimensions(&reduced).unwrap().0, 2000);
    }
}
