//! Imágenes de los documentos con formato: se guardan en una carpeta junto al documento
//! (`notas.md` → `notas_imagenes/`) y se enlazan con una ruta relativa, para que el
//! documento y sus imágenes se puedan mover juntos y se vean igual en otros editores.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use lorca_formats::{parse_image, set_image_target};

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
    let dir = images_dir(document).ok_or(WorkspaceError::NoPath)?;
    import_into(&dir, document.parent(), source)
}

/// Como [`import_image`], para un documento sin guardar: la imagen queda en `scratch`
/// (la carpeta de [`scratch_images_dir`]) hasta que se guarde el documento.
pub fn import_scratch_image(scratch: &Path, source: &Path) -> Result<PathBuf, WorkspaceError> {
    import_into(scratch, Some(scratch), source)
}

/// Deja `source` en `dir` (reducida si hace falta). Si ya está dentro de `keep_in` y no
/// hay que reducirla, se usa tal cual.
fn import_into(
    dir: &Path,
    keep_in: Option<&Path>,
    source: &Path,
) -> Result<PathBuf, WorkspaceError> {
    let source = canonical(source);
    let stem = source
        .file_stem()
        .map_or_else(|| "imagen".to_owned(), |s| s.to_string_lossy().into_owned());
    if needs_reduction(&source) {
        create_dir(dir)?;
        if let Some(reduced) = reduce(&source, dir, &stem)? {
            return Ok(reduced);
        }
    }
    if let Some(keep_in) = keep_in
        && source.starts_with(canonical(keep_in))
    {
        return Ok(source);
    }
    create_dir(dir)?;
    let ext = source
        .extension()
        .map_or_else(|| "png".to_owned(), |e| e.to_string_lossy().to_lowercase());
    let target = unique_path(dir, &stem, &ext);
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
    let dir = images_dir(document).ok_or(WorkspaceError::NoPath)?;
    save_png_into(&dir, width, height, rgba)
}

/// Como [`save_png`], para un documento sin guardar (en `scratch`).
pub fn save_scratch_png(
    scratch: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<PathBuf, WorkspaceError> {
    save_png_into(scratch, width, height, rgba)
}

fn save_png_into(
    dir: &Path,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<PathBuf, WorkspaceError> {
    create_dir(dir)?;
    let target = unique_path(dir, "imagen", "png");
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

/// Crea `dir` si no existe.
fn create_dir(dir: &Path) -> Result<(), WorkspaceError> {
    fs::create_dir_all(dir).map_err(|source| WorkspaceError::Write {
        path: dir.to_path_buf(),
        source,
    })
}

/// Ruta canónica de `path` (o la misma si no existe).
fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Carpeta de las imágenes insertadas en documentos sin guardar: `<datos>/imagenes`. Se
/// enlazan con su ruta absoluta y al guardar el documento pasan a su carpeta de imágenes
/// ([`relocate_images`]).
pub fn scratch_images_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("imagenes")
}

/// Cambios para guardar el documento (hasta ahora en `old`, o sin ruta) en `new`: las
/// imágenes que están en `scratch` o enlazadas con una ruta relativa que desde `new` ya no
/// lleva al mismo archivo se copian a la carpeta de imágenes de `new`, y su línea pasa a
/// enlazar la copia. Devuelve las sustituciones (rangos de líneas en caracteres) en orden.
pub fn relocate_images(
    text: &str,
    old: Option<&Path>,
    new: &Path,
    scratch: Option<&Path>,
) -> Result<Vec<(Range<usize>, String)>, WorkspaceError> {
    let scratch = scratch.map(canonical);
    let mut copies: HashMap<PathBuf, PathBuf> = HashMap::new();
    let mut replacements = Vec::new();
    let mut start = 0;
    for line in text.split('\n') {
        let len = line.chars().count();
        let range = start..start + len;
        start += len + 1;
        let Some(image) = parse_image(line) else {
            continue;
        };
        let Some(source) = resolve_image(old, &image.target).filter(|p| p.is_file()) else {
            continue;
        };
        let source = canonical(&source);
        let in_scratch = scratch.as_ref().is_some_and(|dir| source.starts_with(dir));
        let relative = !Path::new(&image.target).is_absolute() && !image.target.contains("://");
        let left_behind = relative
            && resolve_image(Some(new), &image.target).map(|p| canonical(&p))
                != Some(source.clone());
        if !in_scratch && !left_behind {
            continue;
        }
        let copy = match copies.get(&source) {
            Some(copy) => copy.clone(),
            None => {
                let dir = images_dir(new).ok_or(WorkspaceError::NoPath)?;
                create_dir(&dir)?;
                let stem = source
                    .file_stem()
                    .map_or_else(|| "imagen".to_owned(), |s| s.to_string_lossy().into_owned());
                let ext = source
                    .extension()
                    .map_or_else(|| "png".to_owned(), |e| e.to_string_lossy().to_lowercase());
                let copy = unique_path(&dir, &stem, &ext);
                fs::copy(&source, &copy).map_err(|error| WorkspaceError::Write {
                    path: copy.clone(),
                    source: error,
                })?;
                copies.insert(source, copy.clone());
                copy
            }
        };
        if let Some(line) = set_image_target(line, &image_target(new, &copy)) {
            replacements.push((range, line));
        }
    }
    Ok(replacements)
}

/// Borra de `scratch` las imágenes que no enlaza ninguno de `texts` (documento y su ruta):
/// las de documentos cerrados sin guardar o que ya se guardaron con su documento.
pub fn clean_scratch_images<'a>(
    scratch: &Path,
    texts: impl IntoIterator<Item = (String, Option<&'a Path>)>,
) {
    let Ok(entries) = fs::read_dir(scratch) else {
        return;
    };
    let used: HashSet<PathBuf> = texts
        .into_iter()
        .flat_map(|(text, document)| {
            text.lines()
                .filter_map(parse_image)
                .filter_map(|image| resolve_image(document, &image.target))
                .map(|path| canonical(&path))
                .collect::<Vec<_>>()
        })
        .collect();
    for entry in entries.flatten() {
        let path = canonical(&entry.path());
        if path.is_file() && !used.contains(&path) {
            let _ = fs::remove_file(&path);
        }
    }
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

    #[test]
    fn saving_takes_scratch_images_along() {
        let data = tempfile::tempdir().unwrap();
        let scratch = scratch_images_dir(data.path());
        let pixels = [255_u8, 0, 0, 255].repeat(4);
        let pasted = save_scratch_png(&scratch, 2, 2, &pixels).unwrap();
        assert_eq!(pasted, scratch.join("imagen-1.png"));
        let pasted = image_target(Path::new("/"), &pasted);
        let text = format!("# Notas\n![gato|200]({pasted})\ntexto\n![otra vez]({pasted})");

        let docs = tempfile::tempdir().unwrap();
        let doc = docs.path().join("mis notas.md");
        let edits = relocate_images(&text, None, &doc, Some(&scratch)).unwrap();
        assert_eq!(
            edits,
            vec![
                (
                    8..8 + text.lines().nth(1).unwrap().chars().count(),
                    "![gato|200](<mis notas_imagenes/imagen-1.png>)".to_owned()
                ),
                (
                    text.chars().count() - text.lines().nth(3).unwrap().chars().count()
                        ..text.chars().count(),
                    "![otra vez](<mis notas_imagenes/imagen-1.png>)".to_owned()
                ),
            ]
        );
        // La misma imagen se copia una sola vez.
        let copied: Vec<_> = fs::read_dir(docs.path().join("mis notas_imagenes"))
            .unwrap()
            .collect();
        assert_eq!(copied.len(), 1);
    }

    #[test]
    fn save_as_elsewhere_takes_relative_images_along() {
        let old_dir = tempfile::tempdir().unwrap();
        let old = old_dir.path().join("notas.md");
        let pixels = [255_u8, 0, 0, 255].repeat(4);
        save_png(&old, 2, 2, &pixels).unwrap();
        let text = "![a](notas_imagenes/imagen-1.png)\n![web](https://x.org/a.png)";

        // Al guardar sin más, o en la misma carpeta, el enlace sigue valiendo: no cambia nada.
        assert!(
            relocate_images(text, Some(&old), &old, None)
                .unwrap()
                .is_empty()
        );
        assert!(
            relocate_images(text, Some(&old), &old_dir.path().join("otra.md"), None)
                .unwrap()
                .is_empty()
        );

        let new_dir = tempfile::tempdir().unwrap();
        let new = new_dir.path().join("copia.md");
        let edits = relocate_images(text, Some(&old), &new, None).unwrap();
        assert_eq!(
            edits,
            vec![(0..33, "![a](copia_imagenes/imagen-1.png)".to_owned())]
        );
        assert!(new_dir.path().join("copia_imagenes/imagen-1.png").is_file());
        // El original sigue en su sitio.
        assert!(old_dir.path().join("notas_imagenes/imagen-1.png").is_file());
    }

    #[test]
    fn unused_scratch_images_are_removed() {
        let data = tempfile::tempdir().unwrap();
        let scratch = scratch_images_dir(data.path());
        let pixels = [255_u8, 0, 0, 255].repeat(4);
        let kept = save_scratch_png(&scratch, 2, 2, &pixels).unwrap();
        let gone = save_scratch_png(&scratch, 2, 2, &pixels).unwrap();
        let text = format!("![x]({})", image_target(Path::new("/"), &kept));
        clean_scratch_images(&scratch, [(text, None)]);
        assert!(kept.is_file());
        assert!(!gone.exists());
    }
}
