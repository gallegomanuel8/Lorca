//! Exportación de Markdown a PDF con `genpdf`, usando fuentes del sistema.
//!
//! Las líneas de imagen del texto con formato (`![descripción|300](ruta)`, ver
//! `crate::parse_image`) se incluyen con su ancho y su pie; la ruta la resuelve quien llama.

use std::path::{Path, PathBuf};

use genpdf::elements::{
    Break, FrameCellDecorator, FramedElement, Image, LinearLayout, OrderedList, PaddedElement,
    Paragraph, TableLayout, UnorderedList,
};
use genpdf::fonts::{FontData, FontFamily};
use genpdf::style::{Color, Style};
use genpdf::{Alignment, Element, SimplePageDecorator};
use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Errores al exportar a PDF.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PdfError {
    #[error("no se encontraron fuentes del sistema para el PDF (se buscó: {0})")]
    FontsNotFound(String),
    #[error("no se pudo cargar la fuente «{path}»: {message}")]
    Font { path: String, message: String },
    #[error("no se pudo generar el PDF: {0}")]
    Render(String),
}

/// Archivos de las cuatro variantes de una familia tipográfica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFiles {
    pub regular: PathBuf,
    pub bold: PathBuf,
    pub italic: PathBuf,
    pub bold_italic: PathBuf,
}

/// Fuentes del PDF: una proporcional para el texto y una monoespaciada para el código.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfFonts {
    pub text: FontFiles,
    pub code: FontFiles,
}

impl PdfFonts {
    /// Busca fuentes conocidas del sistema: Arial y Courier New en macOS y Windows,
    /// DejaVu en Linux.
    pub fn system() -> Result<Self, PdfError> {
        let candidates = candidate_fonts();
        candidates
            .iter()
            .find(|fonts| fonts.text.regular.exists() && fonts.code.regular.exists())
            .cloned()
            .ok_or_else(|| {
                let searched: Vec<String> = candidates
                    .iter()
                    .map(|f| f.text.regular.display().to_string())
                    .collect();
                PdfError::FontsNotFound(searched.join(", "))
            })
    }
}

fn family(dir: &str, regular: &str, bold: &str, italic: &str, bold_italic: &str) -> FontFiles {
    let dir = Path::new(dir);
    FontFiles {
        regular: dir.join(regular),
        bold: dir.join(bold),
        italic: dir.join(italic),
        bold_italic: dir.join(bold_italic),
    }
}

fn candidate_fonts() -> Vec<PdfFonts> {
    if cfg!(target_os = "macos") {
        let dir = "/System/Library/Fonts/Supplemental";
        vec![PdfFonts {
            text: family(
                dir,
                "Arial.ttf",
                "Arial Bold.ttf",
                "Arial Italic.ttf",
                "Arial Bold Italic.ttf",
            ),
            code: family(
                dir,
                "Courier New.ttf",
                "Courier New Bold.ttf",
                "Courier New Italic.ttf",
                "Courier New Bold Italic.ttf",
            ),
        }]
    } else if cfg!(windows) {
        let dir = "C:\\Windows\\Fonts";
        vec![PdfFonts {
            text: family(dir, "arial.ttf", "arialbd.ttf", "ariali.ttf", "arialbi.ttf"),
            code: family(dir, "cour.ttf", "courbd.ttf", "couri.ttf", "courbi.ttf"),
        }]
    } else {
        [
            "/usr/share/fonts/truetype/dejavu",
            "/usr/share/fonts/TTF",
            "/usr/share/fonts/dejavu",
        ]
        .iter()
        .map(|dir| PdfFonts {
            text: family(
                dir,
                "DejaVuSans.ttf",
                "DejaVuSans-Bold.ttf",
                "DejaVuSans-Oblique.ttf",
                "DejaVuSans-BoldOblique.ttf",
            ),
            code: family(
                dir,
                "DejaVuSansMono.ttf",
                "DejaVuSansMono-Bold.ttf",
                "DejaVuSansMono-Oblique.ttf",
                "DejaVuSansMono-BoldOblique.ttf",
            ),
        })
        .collect()
    }
}

/// Carga una familia; si falta alguna variante, usa la regular en su lugar.
fn load_family(files: &FontFiles) -> Result<FontFamily<FontData>, PdfError> {
    let read = |path: &Path| -> Result<FontData, PdfError> {
        let error = |message: String| PdfError::Font {
            path: path.display().to_string(),
            message,
        };
        let bytes = std::fs::read(path).map_err(|e| error(e.to_string()))?;
        FontData::new(bytes, None).map_err(|e| error(e.to_string()))
    };
    let regular = read(&files.regular)?;
    let or_regular = |path: &Path| read(path).unwrap_or_else(|_| regular.clone());
    Ok(FontFamily {
        bold: or_regular(&files.bold),
        italic: or_regular(&files.italic),
        bold_italic: or_regular(&files.bold_italic),
        regular,
    })
}

/// Ancho útil de la página en milímetros (A4 menos los márgenes).
const CONTENT_WIDTH_MM: f64 = 170.0;
/// Alto máximo de una imagen en milímetros (que quepa en una página).
const MAX_IMAGE_HEIGHT_MM: f64 = 230.0;
/// Píxeles de pantalla por pulgada con los que se convierte el ancho `|300` a milímetros.
const SCREEN_DPI: f64 = 96.0;
/// Resolución máxima de las imágenes en el PDF: las de más se reducen (pesaría mucho).
const MAX_IMAGE_DPI: f64 = 150.0;
/// Marca que sustituye a cada línea de imagen antes de pasar el texto a pulldown-cmark.
const IMAGE_MARK: char = '\u{E000}';

/// Convierte Markdown en un PDF (tamaño A4) y devuelve sus bytes. `resolve` da el archivo
/// de cada imagen a partir de su ruta escrita (`None` = no se puede mostrar).
pub fn markdown_to_pdf(
    text: &str,
    title: &str,
    fonts: &PdfFonts,
    resolve: &dyn Fn(&str) -> Option<PathBuf>,
) -> Result<Vec<u8>, PdfError> {
    let mut document = genpdf::Document::new(load_family(&fonts.text)?);
    let code_family = document.add_font_family(load_family(&fonts.code)?);
    document.set_title(title);
    document.set_font_size(11);
    document.set_line_spacing(1.25);
    document.set_paper_size(genpdf::PaperSize::A4);
    let mut decorator = SimplePageDecorator::new();
    decorator.set_margins(18);
    document.set_page_decorator(decorator);

    let (text, images) = extract_images(text);
    let mut builder = Builder::new(Style::new().with_font_family(code_family));
    builder.images = images
        .into_iter()
        .map(|image| {
            let file = resolve(&image.target);
            (image, file)
        })
        .collect();
    for event in Parser::new_ext(&text, markdown_options()) {
        builder.event(event);
    }
    document.push(builder.finish());

    let mut out = Vec::new();
    document
        .render(&mut out)
        .map_err(|e| PdfError::Render(e.to_string()))?;
    Ok(out)
}

/// Sustituye cada línea de imagen (fuera de los bloques de código) por un párrafo propio
/// con la marca `IMAGE_MARK` y su número. pulldown-cmark no entiende las rutas con espacios
/// sin `<>` que sí acepta el editor, y así la imagen queda en su sitio.
fn extract_images(text: &str) -> (String, Vec<crate::ImageRef>) {
    let mut out = String::with_capacity(text.len());
    let mut images = Vec::new();
    let mut fence: Option<&str> = None;
    for line in text.split('\n') {
        let trimmed = line.trim_start();
        match fence {
            Some(open) if trimmed.starts_with(open) => fence = None,
            Some(_) => {}
            None if trimmed.starts_with("```") => fence = Some("```"),
            None if trimmed.starts_with("~~~") => fence = Some("~~~"),
            None => {
                if let Some(image) = crate::parse_image(line) {
                    out.push_str(&format!("\n{IMAGE_MARK}{}\n\n", images.len()));
                    images.push(image);
                    continue;
                }
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    (out, images)
}

/// Número de imagen de un texto `IMAGE_MARK` + número.
fn image_index(text: &str) -> Option<usize> {
    text.trim().strip_prefix(IMAGE_MARK)?.parse().ok()
}

/// Carga una imagen para el PDF: sin transparencia (sobre blanco, genpdf no la admite) y
/// con el tamaño en milímetros (`width` en píxeles de pantalla; nunca más ancha que la
/// página).
fn load_image(path: &Path, width: Option<u32>) -> Option<Image> {
    let bytes = std::fs::read(path).ok()?;
    let rgba = image023::load_from_memory(&bytes).ok()?.to_rgba8();
    let (pixels_w, pixels_h) = rgba.dimensions();
    if pixels_w == 0 || pixels_h == 0 {
        return None;
    }
    let screen_width = f64::from(width.unwrap_or(pixels_w).max(1));
    let mut width_mm = (screen_width * 25.4 / SCREEN_DPI).min(CONTENT_WIDTH_MM);
    let height_mm = width_mm * f64::from(pixels_h) / f64::from(pixels_w);
    if height_mm > MAX_IMAGE_HEIGHT_MM {
        width_mm *= MAX_IMAGE_HEIGHT_MM / height_mm;
    }
    // Más píxeles de los que se notan al imprimir: se reduce.
    let max_pixels = (width_mm / 25.4 * MAX_IMAGE_DPI).ceil();
    let rgba = if f64::from(pixels_w) > max_pixels {
        // `max_pixels` es positivo y menor que `pixels_w`: cabe en un `u32`.
        let new_w = (max_pixels as u32).max(1);
        let new_h = u32::try_from(u64::from(pixels_h) * u64::from(new_w) / u64::from(pixels_w))
            .unwrap_or(pixels_h)
            .max(1);
        image023::imageops::resize(
            &rgba,
            new_w,
            new_h,
            image023::imageops::FilterType::Triangle,
        )
    } else {
        rgba
    };
    let (pixels_w, pixels_h) = rgba.dimensions();
    let mut rgb = image023::RgbImage::new(pixels_w, pixels_h);
    for (target, source) in rgb.pixels_mut().zip(rgba.pixels()) {
        let [r, g, b, a] = source.0;
        let blend = |c: u8| {
            let c = u32::from(c) * u32::from(a) + 255 * (255 - u32::from(a));
            u8::try_from(c / 255).unwrap_or(u8::MAX)
        };
        target.0 = [blend(r), blend(g), blend(b)];
    }
    let dpi = f64::from(pixels_w) * 25.4 / width_mm;
    let image = Image::from_dynamic_image(image023::DynamicImage::ImageRgb8(rgb)).ok()?;
    Some(image.with_alignment(Alignment::Center).with_dpi(dpi))
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
}

const LINK_COLOR: Color = Color::Rgb(30, 90, 200);
const MUTED_COLOR: Color = Color::Rgb(110, 110, 110);

/// Lista en construcción.
struct ListState {
    ordered: Option<u64>,
    items: Vec<LinearLayout>,
}

/// Tabla en construcción.
#[derive(Default)]
struct TableState {
    rows: Vec<Vec<Paragraph>>,
    in_head: bool,
}

/// Convierte los eventos de pulldown-cmark en elementos de genpdf.
struct Builder {
    /// Contenedores abiertos (documento, elementos de lista, citas…).
    containers: Vec<LinearLayout>,
    lists: Vec<ListState>,
    table: Option<TableState>,
    paragraph: Option<Paragraph>,
    /// Estilo de cada nivel de formato en línea abierto.
    styles: Vec<Style>,
    code_style: Style,
    code_block: Option<String>,
    link: Option<String>,
    /// Imágenes de las líneas sustituidas por `IMAGE_MARK` y su archivo.
    images: Vec<(crate::ImageRef, Option<PathBuf>)>,
}

impl Builder {
    fn new(code_style: Style) -> Self {
        Self {
            containers: vec![LinearLayout::vertical()],
            lists: Vec::new(),
            table: None,
            paragraph: None,
            styles: vec![Style::new()],
            code_style,
            code_block: None,
            link: None,
            images: Vec::new(),
        }
    }

    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, change: impl FnOnce(Style) -> Style) {
        let style = change(self.style());
        self.styles.push(style);
    }

    fn pop_style(&mut self) {
        if self.styles.len() > 1 {
            self.styles.pop();
        }
    }

    fn push_element<E: Element + 'static>(&mut self, element: E) {
        if let Some(container) = self.containers.last_mut() {
            container.push(element);
        }
    }

    /// Cierra el párrafo actual y lo añade a la celda o al contenedor.
    fn flush_paragraph(&mut self) {
        let Some(paragraph) = self.paragraph.take() else {
            return;
        };
        if let Some(table) = &mut self.table {
            if let Some(row) = table.rows.last_mut() {
                row.push(paragraph);
            }
            return;
        }
        self.push_element(paragraph);
        // Separación entre párrafos (en las listas se dejan juntos).
        if self.lists.is_empty() {
            self.push_element(Break::new(0.5));
        }
    }

    fn text(&mut self, text: &str, style: Style) {
        let paragraph = self.paragraph.get_or_insert_with(|| Paragraph::new(""));
        paragraph.push_styled(text.to_owned(), style);
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => match &mut self.code_block {
                Some(code) => code.push_str(&text),
                None => match image_index(&text) {
                    Some(index) => self.image(index),
                    None => self.text(&text, self.style()),
                },
            },
            Event::Code(code) => {
                let style = self.style().and(self.code_style);
                self.text(&code, style);
            }
            Event::SoftBreak => self.text(" ", self.style()),
            Event::HardBreak => self.flush_paragraph(),
            Event::Rule => {
                self.flush_paragraph();
                self.push_element(Paragraph::new("* * *").aligned(Alignment::Center));
            }
            Event::TaskListMarker(done) => {
                self.text(if done { "[x] " } else { "[ ] " }, self.style());
            }
            Event::FootnoteReference(name) => self.text(&format!("[{name}]"), self.style()),
            Event::Html(html) | Event::InlineHtml(html) => {
                self.text(&html, self.style().with_color(MUTED_COLOR));
            }
            Event::InlineMath(math) | Event::DisplayMath(math) => {
                self.text(&math, self.style().and(self.code_style));
            }
        }
    }

    /// Imagen de una línea propia, con su descripción debajo como pie.
    fn image(&mut self, index: usize) {
        self.flush_paragraph();
        let Some((image, file)) = self.images.get(index) else {
            return;
        };
        let alt = image.alt.trim().to_owned();
        let caption = Style::new().with_font_size(9).with_color(MUTED_COLOR);
        match file
            .as_deref()
            .and_then(|file| load_image(file, image.width))
        {
            Some(element) => {
                self.push_element(PaddedElement::new(element, (1, 0, 1, 0)));
                if !alt.is_empty() {
                    self.push_element(
                        Paragraph::default()
                            .styled_string(alt, caption)
                            .aligned(Alignment::Center),
                    );
                }
            }
            None => {
                let missing = format!("[imagen no disponible: {}]", image.target);
                self.push_element(Paragraph::default().styled_string(missing, caption));
            }
        }
        self.push_element(Break::new(0.5));
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.flush_paragraph(),
            Tag::Heading { level, .. } => {
                self.flush_paragraph();
                self.push_element(Break::new(0.4));
                let size = match level {
                    HeadingLevel::H1 => 22,
                    HeadingLevel::H2 => 18,
                    HeadingLevel::H3 => 15,
                    HeadingLevel::H4 => 13,
                    _ => 12,
                };
                self.push_style(|s| s.bold().with_font_size(size));
            }
            Tag::BlockQuote(_) => {
                self.flush_paragraph();
                self.containers.push(LinearLayout::vertical());
                self.push_style(|s| s.italic().with_color(MUTED_COLOR));
            }
            Tag::CodeBlock(kind) => {
                self.flush_paragraph();
                if let CodeBlockKind::Fenced(language) = kind
                    && !language.is_empty()
                {
                    let label = Paragraph::default().styled_string(
                        language.to_string(),
                        Style::new().with_font_size(8).with_color(MUTED_COLOR),
                    );
                    self.push_element(label);
                }
                self.code_block = Some(String::new());
            }
            Tag::List(start) => {
                self.flush_paragraph();
                self.lists.push(ListState {
                    ordered: start,
                    items: Vec::new(),
                });
            }
            Tag::Item => {
                self.flush_paragraph();
                self.containers.push(LinearLayout::vertical());
            }
            Tag::Table(_) => {
                self.flush_paragraph();
                self.table = Some(TableState::default());
            }
            Tag::TableHead => {
                if let Some(table) = &mut self.table {
                    table.in_head = true;
                    table.rows.push(Vec::new());
                }
                self.push_style(Style::bold);
            }
            Tag::TableRow => {
                if let Some(table) = &mut self.table {
                    table.rows.push(Vec::new());
                }
            }
            Tag::TableCell => self.paragraph = Some(Paragraph::new("")),
            Tag::Emphasis => self.push_style(Style::italic),
            Tag::Strong => self.push_style(Style::bold),
            Tag::Strikethrough => self.push_style(|s| s.with_color(MUTED_COLOR)),
            Tag::Link { dest_url, .. } => {
                self.link = Some(dest_url.to_string());
                self.push_style(|s| s.with_color(LINK_COLOR));
            }
            Tag::Image { dest_url, .. } => {
                self.text(
                    &format!("[imagen: {dest_url}] "),
                    self.style().with_color(MUTED_COLOR),
                );
                self.push_style(|s| s.with_color(MUTED_COLOR));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush_paragraph(),
            TagEnd::Heading(_) => {
                self.flush_paragraph();
                self.pop_style();
                self.push_element(Break::new(0.3));
            }
            TagEnd::BlockQuote(_) => {
                self.flush_paragraph();
                self.pop_style();
                if self.containers.len() > 1
                    && let Some(quote) = self.containers.pop()
                {
                    self.push_element(PaddedElement::new(quote, (0, 0, 0, 8)));
                    self.push_element(Break::new(0.3));
                }
            }
            TagEnd::CodeBlock => {
                let code = self.code_block.take().unwrap_or_default();
                let mut block = LinearLayout::vertical();
                let style = self.code_style.with_font_size(9);
                for line in code.trim_end_matches('\n').lines() {
                    // genpdf no conserva los espacios iniciales: se sustituyen por espacios
                    // no separables.
                    let indent = line.len() - line.trim_start().len();
                    let line = format!("{}{}", "\u{a0}".repeat(indent), line.trim_start());
                    block.push(Paragraph::default().styled_string(line, style));
                }
                self.push_element(PaddedElement::new(
                    FramedElement::new(PaddedElement::new(block, 2)),
                    (1, 0, 2, 0),
                ));
            }
            TagEnd::List(_) => {
                self.flush_paragraph();
                let Some(list) = self.lists.pop() else { return };
                match list.ordered {
                    Some(start) => {
                        let mut ordered =
                            OrderedList::with_start(usize::try_from(start).unwrap_or(1));
                        list.items.into_iter().for_each(|item| ordered.push(item));
                        self.push_element(ordered);
                    }
                    None => {
                        let mut unordered = UnorderedList::new();
                        list.items.into_iter().for_each(|item| unordered.push(item));
                        self.push_element(unordered);
                    }
                }
                if self.lists.is_empty() {
                    self.push_element(Break::new(0.5));
                }
            }
            TagEnd::Item => {
                self.flush_paragraph();
                if self.containers.len() > 1
                    && let Some(item) = self.containers.pop()
                    && let Some(list) = self.lists.last_mut()
                {
                    list.items.push(item);
                }
            }
            TagEnd::TableHead => {
                self.flush_paragraph();
                self.pop_style();
                if let Some(table) = &mut self.table {
                    table.in_head = false;
                }
            }
            TagEnd::TableCell => self.flush_paragraph(),
            TagEnd::Table => {
                let Some(table) = self.table.take() else {
                    return;
                };
                let columns = table.rows.iter().map(Vec::len).max().unwrap_or(0);
                if columns == 0 {
                    return;
                }
                let mut layout = TableLayout::new(vec![1; columns]);
                layout.set_cell_decorator(FrameCellDecorator::new(true, true, false));
                for row in table.rows {
                    let mut cells: Vec<Box<dyn Element>> = row
                        .into_iter()
                        .map(|cell| Box::new(PaddedElement::new(cell, 1)) as Box<dyn Element>)
                        .collect();
                    while cells.len() < columns {
                        cells.push(Box::new(Paragraph::new("")));
                    }
                    // Una fila mal formada no debe impedir exportar el resto.
                    let _ = layout.push_row(cells);
                }
                self.push_element(PaddedElement::new(layout, (1, 0, 2, 0)));
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => self.pop_style(),
            TagEnd::Link => {
                self.pop_style();
                if let Some(url) = self.link.take()
                    && !url.starts_with('#')
                {
                    self.text(
                        &format!(" ({url})"),
                        self.style().with_color(MUTED_COLOR).with_font_size(9),
                    );
                }
            }
            TagEnd::Image => self.pop_style(),
            _ => {}
        }
    }

    fn finish(mut self) -> LinearLayout {
        self.flush_paragraph();
        while self.containers.len() > 1 {
            if let Some(inner) = self.containers.pop() {
                self.push_element(inner);
            }
        }
        self.containers.pop().unwrap_or_else(LinearLayout::vertical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Romancero gitano\n\n*Verde* que te quiero **verde**, `código`.\n\n\
        > Una cita\n\n- uno\n- dos\n  1. anidado\n\n```rust\nfn main() {\n    println!(\"hola\");\n}\n```\n\n\
        | Obra | Año |\n|---|---|\n| Yerma | 1934 |\n\n[enlace](https://example.com)\n\n---\n";

    #[test]
    fn exports_a_pdf_when_system_fonts_exist() {
        // En máquinas sin las fuentes esperadas (p. ej. algunos CI) no hay nada que probar.
        let Ok(fonts) = PdfFonts::system() else {
            return;
        };
        let pdf = markdown_to_pdf(SAMPLE, "Prueba", &fonts, &|_| None).unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        assert!(pdf.len() > 1000);
    }

    #[test]
    fn image_lines_are_extracted_outside_code() {
        let (text, images) =
            extract_images("hola\n![foto de mar|300](<mis fotos/a.png>)\n```\n![no](b.png)\n```\n");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].target, "mis fotos/a.png");
        assert_eq!(images[0].width, Some(300));
        assert!(text.contains(&format!("\n{IMAGE_MARK}0\n\n")));
        assert!(text.contains("![no](b.png)"));
        assert_eq!(image_index(&format!("{IMAGE_MARK}0")), Some(0));
        assert_eq!(image_index("0"), None);
    }

    #[test]
    fn exports_images_with_transparency() {
        let Ok(fonts) = PdfFonts::system() else {
            return;
        };
        let dir = std::env::temp_dir().join(format!("lorca-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("punto.png");
        let mut png = image023::RgbaImage::new(40, 20);
        png.put_pixel(1, 1, image023::Rgba([200, 0, 0, 128]));
        png.save(&file).unwrap();
        let resolve = |target: &str| Some(dir.join(target));
        let with =
            markdown_to_pdf("# A\n![punto|200](punto.png)\n", "x", &fonts, &resolve).unwrap();
        let without = markdown_to_pdf("# A\n", "x", &fonts, &resolve).unwrap();
        assert!(with.len() > without.len());
        // Si no se puede cargar, se dice en el PDF en vez de fallar.
        assert!(markdown_to_pdf("![x](no.png)\n", "x", &fonts, &resolve).is_ok());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_fonts_are_reported() {
        let missing = FontFiles {
            regular: PathBuf::from("/no/existe.ttf"),
            bold: PathBuf::from("/no/existe.ttf"),
            italic: PathBuf::from("/no/existe.ttf"),
            bold_italic: PathBuf::from("/no/existe.ttf"),
        };
        let fonts = PdfFonts {
            text: missing.clone(),
            code: missing,
        };
        assert!(matches!(
            markdown_to_pdf("hola", "x", &fonts, &|_| None),
            Err(PdfError::Font { .. })
        ));
    }
}
