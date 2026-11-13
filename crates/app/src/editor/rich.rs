//! Edición con formato (WYSIWYG) para los documentos de texto plano: como el Bloc de notas
//! de Windows 11, el texto se ve con letra proporcional, títulos más grandes, viñetas,
//! negrita, cursiva, tachado y vínculos, sin las marcas de Markdown (que siguen en el
//! texto). Las tablas se ven tal cual, en monoespaciada.
//!
//! Igual que el editor de código, el widget solo lee [`EditorState`] y publica
//! [`EditorAction`]. Cada línea del documento es un párrafo de iced (con ajuste de línea)
//! que se guarda en caché mientras su texto no cambie. Las posiciones se traducen entre
//! columnas del texto y bytes del párrafo (las marcas ocultas no ocupan sitio).
//!
//! Una línea con solo una imagen (`![descripción](ruta)`, [`Block::Image`]) muestra la
//! imagen (ajustada al ancho, sin agrandarla) y debajo la descripción como pie: el párrafo
//! de la línea, donde se coloca el cursor, empieza en `text_top`. Un clic en la imagen
//! selecciona su línea entera (se ve con un marco) y arrastrarla la mueve a otro hueco
//! entre líneas (`EditorAction::MoveLine`). Con la imagen seleccionada, el tirador de su
//! esquina inferior derecha cambia su ancho (`EditorAction::ResizeImage`, que lo guarda como
//! `![descripción|300](ruta)`); doble clic en él la deja a su tamaño.

use std::path::{Path, PathBuf};

use iced::advanced::graphics::text::cosmic_text;
use iced::advanced::image::{self as advanced_image, Renderer as _};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, Paragraph as _, Renderer as _, Span, Text};
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, InputMethod, Shell, clipboard, input_method, mouse};
use iced::alignment;
use iced::keyboard;
use iced::time::{Duration, Instant};
use iced::window;
use iced::{
    Background, Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Renderer, Size,
    Theme,
};
use lorca_core::{EditorState, Motion};
use lorca_formats::{Block, CharStyle, LineMarkup, parse_image, parse_line};

use super::keymap::{KeyCommand, map_key};
use super::palette::EditorColors;
use super::{ClickKind, DocView, EditorAction, Metrics, Viewport};

type Paragraph = <Renderer as text::Renderer>::Paragraph;

/// Margen alrededor del texto.
const PADDING: f32 = 24.0;
/// Anchura de la barra de desplazamiento (solo indicador).
const SCROLLBAR: f32 = 8.0;
/// Periodo de parpadeo del cursor.
const BLINK_MILLIS: u128 = 530;
/// Tamaño de los títulos respecto al texto (niveles 1 a 6).
const HEADING_SCALE: [f32; 6] = [2.0, 1.6, 1.35, 1.15, 1.0, 0.9];
/// Altura de línea relativa al tamaño de letra.
const LINE_HEIGHT: f32 = 1.45;
/// Tamaño del pie de las imágenes respecto al texto.
const CAPTION_SCALE: f32 = 0.85;
/// Separación entre una imagen y su pie.
const IMAGE_GAP: f32 = 6.0;
/// Distancia que hay que mover el ratón para empezar a arrastrar una imagen.
const DRAG_THRESHOLD: f32 = 4.0;
/// Anchura máxima de la copia translúcida que sigue al ratón al arrastrar una imagen.
const GHOST_WIDTH: f32 = 200.0;
/// Lado del tirador para cambiar el tamaño de una imagen.
const HANDLE: f32 = 10.0;
/// Ancho mínimo de una imagen al cambiar su tamaño.
const MIN_IMAGE_WIDTH: f32 = 32.0;

/// Crea el editor con formato de un documento.
pub fn rich_editor<'a, Message>(
    editor: &'a EditorState,
    document: Option<&'a Path>,
    view: &'a DocView,
    focused: bool,
    colors: &'a EditorColors,
    metrics: Metrics,
    on_action: impl Fn(EditorAction) -> Message + 'a,
) -> RichEditor<'a, Message> {
    RichEditor {
        editor,
        document,
        view,
        focused,
        colors,
        metrics,
        on_action: Box::new(on_action),
    }
}

pub struct RichEditor<'a, Message> {
    editor: &'a EditorState,
    /// Ruta del documento: las imágenes con ruta relativa se buscan junto a él.
    document: Option<&'a Path>,
    view: &'a DocView,
    focused: bool,
    colors: &'a EditorColors,
    metrics: Metrics,
    on_action: Box<dyn Fn(EditorAction) -> Message + 'a>,
}

impl<Message> std::fmt::Debug for RichEditor<'_, Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RichEditor")
            .field("selection", &self.editor.selection())
            .finish_non_exhaustive()
    }
}

/// Una línea del documento ya maquetada.
struct LineLayout {
    text: String,
    paragraph: Paragraph,
    /// Byte del párrafo para cada columna del texto (una más para el final).
    col_byte: Vec<usize>,
    markup: LineMarkup,
    height: f32,
    /// Imagen de la línea (solo en [`Block::Image`] con la imagen cargada).
    image: Option<LineImage>,
    /// Altura a la que empieza el párrafo dentro de la línea (debajo de la imagen).
    text_top: f32,
    /// Tramos tachados o subrayados (índice del tramo, tachado, color o `None` para el
    /// del texto): iced no dibuja esas líneas al pintar un párrafo.
    lines_on: Vec<(usize, bool, Option<Color>)>,
}

/// Imagen ya cargada de una línea.
struct LineImage {
    /// Mantiene la imagen en la caché del renderizador mientras se usa.
    allocation: advanced_image::Allocation,
    size: Size,
}

/// Imagen pulsada (y quizá arrastrada).
struct ImageDrag {
    line: usize,
    /// Dónde se pulsó y dónde está el ratón (pantalla).
    start: Point,
    position: Point,
    /// Ya se ha movido lo bastante para considerarlo un arrastre.
    moving: bool,
    /// Hueco entre líneas donde caería.
    drop_at: Option<usize>,
}

/// Cambio de tamaño de una imagen con su tirador.
struct ImageResize {
    line: usize,
    /// Dónde se pulsó (pantalla) y el ancho que tenía.
    start: Point,
    start_width: f32,
    /// Ancho mientras se arrastra.
    width: f32,
}

struct State {
    lines: Vec<LineLayout>,
    /// Posición vertical de cada línea (una más: el alto total).
    tops: Vec<f32>,
    /// Versión del documento, anchura y tamaño de letra de la maquetación.
    built_for: Option<(u64, f32, f32)>,
    /// Ruta del documento de la maquetación (de ella dependen las imágenes).
    built_path: Option<PathBuf>,
    last_click: Option<mouse::Click>,
    dragging: bool,
    modifiers: keyboard::Modifiers,
    blink_epoch: Instant,
    now: Instant,
    window_focused: bool,
    reported_viewport: Option<Viewport>,
    reported_height: Option<f32>,
    /// Última (versión, cursor) cuya visibilidad se aseguró.
    revealed: Option<(u64, usize)>,
    /// Posición horizontal que mantienen ↑/↓.
    goal_x: Option<f32>,
    /// Rueda acumulada para el zoom (ver `zoom_steps`).
    zoom_pending: f32,
    image_drag: Option<ImageDrag>,
    image_resize: Option<ImageResize>,
}

impl Default for State {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            lines: Vec::new(),
            tops: vec![0.0],
            built_for: None,
            built_path: None,
            last_click: None,
            dragging: false,
            modifiers: keyboard::Modifiers::default(),
            blink_epoch: now,
            now,
            window_focused: true,
            reported_viewport: None,
            reported_height: None,
            revealed: None,
            goal_x: None,
            zoom_pending: 0.0,
            image_drag: None,
            image_resize: None,
        }
    }
}

impl State {
    fn total_height(&self) -> f32 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    /// Línea que ocupa la altura `y` (relativa al contenido).
    fn line_at(&self, y: f32) -> usize {
        let line = self.tops.partition_point(|top| *top <= y).saturating_sub(1);
        line.min(self.lines.len().saturating_sub(1))
    }

    /// Rectángulo del cursor en la columna `col` de `line` (coordenadas del contenido).
    fn caret(&self, line: usize, col: usize) -> Option<Rectangle> {
        let layout = self.lines.get(line)?;
        let top = self.tops.get(line).copied()?;
        let byte = layout.col_byte.get(col).copied().unwrap_or(0);
        let (x, y, height) = caret_in(layout, byte)?;
        Some(Rectangle {
            x,
            y: top + layout.text_top + y,
            width: 2.0,
            height,
        })
    }

    /// Índice de carácter del documento bajo un punto del contenido.
    fn index_at(&self, editor: &EditorState, point: Point) -> usize {
        let buffer = editor.buffer();
        let line = self.line_at(point.y.max(0.0));
        let Some(layout) = self.lines.get(line) else {
            return buffer.len_chars();
        };
        let top = self.tops.get(line).copied().unwrap_or(0.0);
        let text_height = (layout.height - layout.text_top).max(1.0);
        let local_y = (point.y - top - layout.text_top).clamp(0.0, text_height - 1.0);
        let byte = layout
            .paragraph
            .buffer()
            .hit(point.x.max(0.0), local_y)
            .map_or(0, |cursor| cursor.index);
        // La primera columna que cae en ese byte (antes de las marcas que la siguen).
        let col = layout
            .col_byte
            .iter()
            .position(|b| *b >= byte)
            .unwrap_or(layout.col_byte.len().saturating_sub(1));
        let col = layout.markup.snap(col, true);
        buffer.line_start(line) + col
    }

    /// Rectángulo de la imagen de `line` (coordenadas del contenido).
    fn image_rect(&self, line: usize) -> Option<Rectangle> {
        let image = self.lines.get(line)?.image.as_ref()?;
        let top = self.tops.get(line).copied()?;
        Some(Rectangle::new(Point::new(0.0, top), image.size))
    }

    /// Línea cuya imagen está bajo un punto del contenido.
    fn image_at(&self, point: Point) -> Option<usize> {
        let line = self.line_at(point.y.max(0.0));
        self.image_rect(line)
            .is_some_and(|rect| rect.contains(point))
            .then_some(line)
    }

    /// Hueco entre líneas más cercano a la altura `y` (0 = antes de la primera).
    fn drop_slot(&self, y: f32) -> usize {
        let line = self.line_at(y.max(0.0));
        let top = self.tops.get(line).copied().unwrap_or(0.0);
        let height = self.lines.get(line).map_or(0.0, |l| l.height);
        if y < top + height / 2.0 {
            line
        } else {
            (line + 1).min(self.lines.len())
        }
    }

    fn restart_blink(&mut self) {
        self.blink_epoch = Instant::now();
        self.now = self.blink_epoch;
    }

    fn cursor_visible(&self) -> bool {
        let elapsed = self
            .now
            .saturating_duration_since(self.blink_epoch)
            .as_millis();
        (elapsed / BLINK_MILLIS).is_multiple_of(2)
    }
}

/// Posición (x, y relativa a la línea, alto) del cursor en el byte `byte` de un párrafo.
fn caret_in(layout: &LineLayout, byte: usize) -> Option<(f32, f32, f32)> {
    let buffer = layout.paragraph.buffer();
    let mut end_of_run = None;
    let mut last = None;
    for run in buffer.layout_runs() {
        last = Some((run.line_w, run.line_top, run.line_height));
        let Some(first) = run.glyphs.first() else {
            if byte == 0 {
                return Some((0.0, run.line_top, run.line_height));
            }
            continue;
        };
        let end = run.glyphs.iter().map(|g| g.end).max().unwrap_or(first.end);
        if let Some(glyph) = run.glyphs.iter().find(|g| g.start <= byte && byte < g.end) {
            return Some((glyph.x, run.line_top, run.line_height));
        }
        if byte == end && end_of_run.is_none() {
            let right = run.glyphs.iter().map(|g| g.x + g.w).fold(0.0_f32, f32::max);
            end_of_run = Some((right, run.line_top, run.line_height));
        }
    }
    end_of_run.or(last)
}

/// Familia del texto: una concreta por sistema, para que la negrita y la cursiva sean de
/// la misma familia (con «sans-serif» genérico, la negrita caía en otra letra en macOS).
pub const PROSE_FAMILY: iced::font::Family = if cfg!(target_os = "macos") {
    iced::font::Family::Name("Helvetica Neue")
} else if cfg!(windows) {
    iced::font::Family::Name("Segoe UI")
} else {
    iced::font::Family::SansSerif
};

/// Estilo de letra de un carácter según su bloque.
fn font_for(block: Block, style: CharStyle) -> Font {
    if style.code || block == Block::Raw {
        return Font::MONOSPACE;
    }
    let bold = style.bold || matches!(block, Block::Heading(_));
    Font {
        weight: if bold {
            iced::font::Weight::Bold
        } else {
            iced::font::Weight::Normal
        },
        style: if style.italic {
            iced::font::Style::Italic
        } else {
            iced::font::Style::Normal
        },
        family: PROSE_FAMILY,
        ..Font::default()
    }
}

/// Tamaño de letra de una línea.
fn size_for(block: Block, base: f32) -> f32 {
    match block {
        Block::Heading(level) => {
            base * HEADING_SCALE
                .get(usize::from(level.saturating_sub(1)))
                .copied()
                .unwrap_or(1.0)
        }
        Block::Raw => base * 0.95,
        _ => base,
    }
}

/// Imagen de una línea [`Block::Image`]: la cargada, o el aviso que se pone en el pie si
/// no se puede mostrar.
fn line_image(
    text: &str,
    document: Option<&Path>,
    renderer: &Renderer,
) -> Result<LineImage, String> {
    let image = parse_image(text).ok_or_else(String::new)?;
    let Some(path) = lorca_workspace::resolve_image(document, &image.target) else {
        return Err(if image.target.contains("://") {
            format!("imagen de internet: {}", image.target)
        } else {
            format!("guarda el documento para ver «{}»", image.target)
        });
    };
    if !path.is_file() {
        return Err(format!("no se encuentra «{}»", image.target));
    }
    let handle = advanced_image::Handle::from_path(path);
    let allocation = renderer
        .load_image(&handle)
        .map_err(|_| format!("no se puede abrir «{}»", image.target))?;
    let size = allocation.size();
    let natural = Size::new(size.width as f32, size.height as f32);
    // Con ancho propio (`|300`) se escala a él, manteniendo la proporción.
    let size = match image.width {
        Some(width) if natural.width > 0.0 => {
            let width = width as f32;
            Size::new(width, natural.height * width / natural.width)
        }
        _ => natural,
    };
    Ok(LineImage { size, allocation })
}

/// Maqueta una línea para la anchura `width`.
fn build_line(
    text: &str,
    width: f32,
    base: f32,
    colors: &EditorColors,
    document: Option<&Path>,
    renderer: &Renderer,
) -> LineLayout {
    let markup = parse_line(text);
    let is_image = markup.block == Block::Image;
    let (image, image_note) = if is_image {
        match line_image(text, document, renderer) {
            Ok(image) => (Some(image), None),
            Err(note) => (None, Some(note)),
        }
    } else {
        (None, None)
    };
    let size = if is_image {
        base * CAPTION_SCALE
    } else {
        size_for(markup.block, base)
    };
    let mut display = String::new();
    let mut col_byte = Vec::with_capacity(markup.chars.len() + 1);
    // Tramos: (inicio en bytes, estilo).
    let mut runs: Vec<(usize, CharStyle)> = Vec::new();
    for (c, style) in text.chars().zip(markup.chars.iter().copied()) {
        col_byte.push(display.len());
        if style.hidden {
            continue;
        }
        let key = CharStyle {
            hidden: false,
            bullet: false,
            ..style
        };
        if runs.last().is_none_or(|(_, last)| *last != key) {
            runs.push((display.len(), key));
        }
        display.push(if style.bullet { '•' } else { c });
    }
    col_byte.push(display.len());

    let spans: Vec<Span<'static, (), Font>> = runs
        .iter()
        .enumerate()
        .map(|(index, (start, style))| {
            let end = runs.get(index + 1).map_or(display.len(), |(next, _)| *next);
            let mut span = Span::new(display.get(*start..end).unwrap_or_default().to_owned())
                .font(font_for(markup.block, *style))
                .size(size)
                .strikethrough(style.strike)
                .underline(style.link);
            if is_image {
                span = span.color(colors.line_number).font(Font {
                    style: iced::font::Style::Italic,
                    ..font_for(markup.block, *style)
                });
            } else if style.link {
                span = span.color(colors.bookmark);
            } else if style.code {
                span = span.color(colors.line_number_active);
            }
            span
        })
        .collect();
    // El aviso de una imagen que no se ve va detrás de la descripción (fuera de las
    // columnas del texto: el cursor no entra en él).
    let mut spans = spans;
    if let Some(note) = image_note {
        let gap = if display.is_empty() { "" } else { " — " };
        spans.push(
            Span::new(format!("{gap}🖼 {note}"))
                .size(size)
                .color(colors.line_number)
                .font(font_for(Block::Paragraph, CharStyle::default())),
        );
    }
    let lines_on = runs
        .iter()
        .enumerate()
        .filter(|(_, (_, style))| style.strike || style.link)
        .map(|(index, (_, style))| (index, style.strike, style.link.then_some(colors.bookmark)))
        .collect();
    let paragraph = Paragraph::with_spans(Text {
        content: spans.as_slice(),
        bounds: Size::new(width.max(10.0), f32::INFINITY),
        size: Pixels(size),
        line_height: text::LineHeight::Relative(LINE_HEIGHT),
        font: font_for(markup.block, CharStyle::default()),
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::WordOrGlyph,
    });
    let min_height = size * LINE_HEIGHT;
    // Un poco de aire debajo de los títulos.
    let extra = if matches!(markup.block, Block::Heading(_)) {
        size * 0.25
    } else {
        0.0
    };
    // La imagen, ajustada al ancho (sin agrandarla) y encima del pie.
    let image = image.map(|image| {
        let scale = (width / image.size.width.max(1.0)).min(1.0);
        LineImage {
            size: Size::new(image.size.width * scale, image.size.height * scale),
            ..image
        }
    });
    let text_top = image
        .as_ref()
        .map_or(0.0, |image| image.size.height + IMAGE_GAP);
    let extra = if is_image { size * 0.5 } else { extra };
    let height = text_top + paragraph.min_bounds().height.max(min_height) + extra;
    LineLayout {
        text: text.to_owned(),
        paragraph,
        col_byte,
        markup,
        height,
        image,
        text_top,
        lines_on,
    }
}

impl<Message> RichEditor<'_, Message> {
    fn publish(&self, shell: &mut Shell<'_, Message>, action: EditorAction) {
        shell.publish((self.on_action)(action));
    }

    fn text_width(bounds: Rectangle) -> f32 {
        (bounds.width - 2.0 * PADDING - SCROLLBAR).max(10.0)
    }

    /// Rehace la maquetación si cambió el documento, la anchura o la letra. Reaprovecha
    /// las líneas cuyo texto no ha cambiado.
    fn rebuild(&self, state: &mut State, width: f32, renderer: &Renderer) {
        let key = (self.editor.version(), width, self.metrics.font_size);
        let same_path = state.built_path.as_deref() == self.document;
        if state.built_for == Some(key) && same_path {
            return;
        }
        let same_metrics = same_path
            && state
                .built_for
                .is_some_and(|(_, w, size)| w == width && size == self.metrics.font_size);
        let mut old: std::collections::HashMap<String, Vec<LineLayout>> =
            std::collections::HashMap::new();
        if same_metrics {
            for line in std::mem::take(&mut state.lines) {
                old.entry(line.text.clone()).or_default().push(line);
            }
        }
        let buffer = self.editor.buffer();
        let mut lines = Vec::with_capacity(buffer.len_lines());
        for line in 0..buffer.len_lines() {
            let text = buffer.line_text(line);
            let layout = old.get_mut(&text).and_then(Vec::pop).unwrap_or_else(|| {
                build_line(
                    &text,
                    width,
                    self.metrics.font_size,
                    self.colors,
                    self.document,
                    renderer,
                )
            });
            lines.push(layout);
        }
        let mut tops = Vec::with_capacity(lines.len() + 1);
        let mut y = 0.0;
        tops.push(y);
        for line in &lines {
            y += line.height;
            tops.push(y);
        }
        state.lines = lines;
        state.tops = tops;
        state.built_for = Some(key);
        state.built_path = self.document.map(Path::to_path_buf);
    }

    /// ¿Está seleccionada entera la línea de imagen `line`?
    fn is_image_selected(&self, state: &State, line: usize) -> bool {
        if state.image_rect(line).is_none() {
            return false;
        }
        let selection = self.editor.selection().range();
        let start = self.editor.buffer().line_start(line);
        let len = state.lines.get(line).map_or(0, |l| l.text.chars().count());
        !selection.is_empty() && selection.start <= start && selection.end >= start + len
    }

    /// Línea de la imagen seleccionada cuyo tirador está bajo un punto del contenido.
    fn handle_at(&self, state: &State, point: Point) -> Option<usize> {
        let line = state.line_at(point.y.max(0.0));
        // El tirador sobresale un poco por debajo de la imagen.
        let candidates = [line, line.saturating_sub(1)];
        candidates.into_iter().find(|&line| {
            self.is_image_selected(state, line)
                && state
                    .image_rect(line)
                    .is_some_and(|rect| handle_rect(rect).expand(3.0).contains(point))
        })
    }

    /// Esquina del contenido en pantalla.
    fn origin(&self, bounds: Rectangle) -> Point {
        Point::new(bounds.x + PADDING, bounds.y + PADDING - self.view.scroll_y)
    }

    /// Cursor principal en coordenadas del contenido.
    fn head_caret(&self, state: &State) -> Option<Rectangle> {
        let head = self.editor.selection().head;
        let position = self.editor.buffer().char_to_position(head);
        state.caret(position.line, position.column)
    }

    /// Destino de mover el cursor `rows` líneas visuales (negativo: arriba).
    fn vertical_target(&self, state: &mut State, step: f32) -> Option<usize> {
        let caret = self.head_caret(state)?;
        let x = *state.goal_x.get_or_insert(caret.x);
        let y = if step < 0.0 {
            caret.y + step
        } else {
            caret.y + caret.height + step - 1.0
        };
        if y < 0.0 {
            return Some(0);
        }
        if y >= state.total_height() {
            return Some(self.editor.buffer().len_chars());
        }
        Some(state.index_at(self.editor, Point::new(x, y)))
    }
}

impl<Message> Widget<Message, Theme, Renderer> for RichEditor<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        let width = Self::text_width(Rectangle::new(Point::ORIGIN, limits.max()));
        self.rebuild(state, width, renderer);
        layout::atomic(limits, Length::Fill, Length::Fill)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let origin = self.origin(bounds);
        let to_content = |point: Point| Point::new(point.x - origin.x, point.y - origin.y);
        let visible_height = (bounds.height - 2.0 * PADDING).max(0.0);

        match event {
            Event::Window(window::Event::Focused) => {
                state.window_focused = true;
                state.restart_blink();
                shell.request_redraw();
            }
            Event::Window(window::Event::Unfocused) => {
                state.window_focused = false;
                state.dragging = false;
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                state.now = *now;
                let viewport = Viewport {
                    width: Self::text_width(bounds),
                    height: visible_height,
                    cell_width: self.metrics.font_size * 0.6,
                    metrics: self.metrics,
                };
                if state.reported_viewport != Some(viewport) {
                    state.reported_viewport = Some(viewport);
                    self.publish(shell, EditorAction::Viewport(viewport));
                }
                let height = state.total_height();
                if state.reported_height != Some(height) {
                    state.reported_height = Some(height);
                    self.publish(shell, EditorAction::RichHeight(height));
                }
                // Tras mover el cursor o editar, que se vea (con la maquetación nueva).
                let key = (self.editor.version(), self.editor.selection().head);
                if state.revealed != Some(key) {
                    state.revealed = Some(key);
                    if let Some(caret) = self.head_caret(state) {
                        let scroll = self.view.scroll_y;
                        if caret.y < scroll {
                            self.publish(
                                shell,
                                EditorAction::ScrollTo {
                                    x: None,
                                    y: Some(caret.y),
                                },
                            );
                        } else if caret.y + caret.height > scroll + visible_height {
                            let y = caret.y + caret.height - visible_height;
                            self.publish(
                                shell,
                                EditorAction::ScrollTo {
                                    x: None,
                                    y: Some(y),
                                },
                            );
                        }
                        if self.focused {
                            let point =
                                Point::new(origin.x + caret.x, origin.y + caret.y + caret.height);
                            self.publish(shell, EditorAction::CursorPoint(point));
                        }
                    }
                }
                if self.focused && state.window_focused {
                    let elapsed = now.saturating_duration_since(state.blink_epoch).as_millis();
                    let wait = BLINK_MILLIS - elapsed % BLINK_MILLIS;
                    let wait = u64::try_from(wait).unwrap_or(1);
                    shell.request_redraw_at(*now + Duration::from_millis(wait));
                }
                let input: InputMethod<&str> = if self.focused && state.window_focused {
                    let rect = self.head_caret(state).map_or(bounds, |caret| Rectangle {
                        x: origin.x + caret.x,
                        y: origin.y + caret.y,
                        ..caret
                    });
                    InputMethod::Enabled {
                        cursor: rect,
                        purpose: input_method::Purpose::Normal,
                        preedit: None,
                    }
                } else {
                    InputMethod::Disabled
                };
                shell.request_input_method(&input);
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                text,
                ..
            }) if self.focused && !shell.is_event_captured() => {
                state.modifiers = *modifiers;
                let line_step = self.metrics.font_size * LINE_HEIGHT * 0.5;
                let Some(command) = map_key(key, *physical_key, *modifiers, text.as_deref(), 1)
                else {
                    return;
                };
                match command {
                    // ↑/↓ y Re Pág/Av Pág se mueven por líneas visuales (con títulos de otro
                    // tamaño, las líneas del texto no sirven).
                    KeyCommand::Edit(EditorAction::Move { motion, extend })
                        if matches!(
                            motion,
                            Motion::Up | Motion::Down | Motion::PageUp(_) | Motion::PageDown(_)
                        ) =>
                    {
                        let step = match motion {
                            Motion::Up => -line_step,
                            Motion::Down => line_step,
                            Motion::PageUp(_) => -visible_height,
                            _ => visible_height,
                        };
                        if let Some(index) = self.vertical_target(state, step) {
                            self.publish(
                                shell,
                                EditorAction::Click {
                                    index,
                                    kind: ClickKind::Single,
                                    extend,
                                },
                            );
                        }
                    }
                    KeyCommand::Edit(EditorAction::CollapseSelection)
                        if self.editor.selection().is_empty()
                            && self.editor.selections().len() == 1 =>
                    {
                        return;
                    }
                    KeyCommand::Edit(action) => {
                        state.goal_x = None;
                        self.publish(shell, action);
                    }
                    KeyCommand::Copy => {
                        let selected = self.editor.selected_texts();
                        if !selected.is_empty() {
                            clipboard.write(clipboard::Kind::Standard, selected.join("\n"));
                        }
                    }
                    KeyCommand::Cut => {
                        let selected = self.editor.selected_texts();
                        if !selected.is_empty() {
                            clipboard.write(clipboard::Kind::Standard, selected.join("\n"));
                            self.publish(shell, EditorAction::DeleteSelection);
                        }
                    }
                    KeyCommand::Paste => {
                        match clipboard.read(clipboard::Kind::Standard) {
                            Some(contents) => self.publish(shell, EditorAction::Paste(contents)),
                            // Sin texto puede haber una imagen (la app lo mira).
                            None => self.publish(shell, EditorAction::PasteImage),
                        }
                    }
                }
                state.restart_blink();
                shell.capture_event();
            }
            Event::InputMethod(input_method::Event::Commit(content))
                if self.focused && !shell.is_event_captured() =>
            {
                self.publish(shell, EditorAction::Insert(content.clone()));
                state.restart_blink();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.position_over(bounds).is_some_and(|position| {
                    self.handle_at(state, to_content(position)).is_some()
                }) =>
            {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };
                let Some(line) = self.handle_at(state, to_content(position)) else {
                    return;
                };
                let click = mouse::Click::new(position, mouse::Button::Left, state.last_click);
                state.last_click = Some(click);
                if click.kind() == mouse::click::Kind::Double {
                    // Doble clic: tamaño original.
                    state.image_resize = None;
                    self.publish(shell, EditorAction::ResizeImage { line, width: None });
                } else if let Some(rect) = state.image_rect(line) {
                    state.image_resize = Some(ImageResize {
                        line,
                        start: position,
                        start_width: rect.width,
                        width: rect.width,
                    });
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position })
                if state.image_resize.is_some() =>
            {
                let max = Self::text_width(bounds);
                if let Some(resize) = &mut state.image_resize {
                    resize.width = (resize.start_width + position.x - resize.start.x)
                        .clamp(MIN_IMAGE_WIDTH.min(max), max);
                }
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.image_resize.is_some() =>
            {
                if let Some(resize) = state.image_resize.take()
                    && (resize.width - resize.start_width).abs() >= 1.0
                {
                    self.publish(
                        shell,
                        EditorAction::ResizeImage {
                            line: resize.line,
                            width: Some(resize.width.round() as u32),
                        },
                    );
                }
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor
                    .position_over(bounds)
                    .is_some_and(|position| state.image_at(to_content(position)).is_some()) =>
            {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };
                let Some(line) = state.image_at(to_content(position)) else {
                    return;
                };
                // La línea entera, salto incluido; el cursor al principio.
                let buffer = self.editor.buffer();
                let start = buffer.line_start(line);
                let end = if line + 1 < buffer.len_lines() {
                    buffer.line_start(line + 1)
                } else {
                    buffer.len_chars()
                };
                self.publish(
                    shell,
                    EditorAction::SelectRange {
                        anchor: end,
                        head: start,
                    },
                );
                state.image_drag = Some(ImageDrag {
                    line,
                    start: position,
                    position,
                    moving: false,
                    drop_at: None,
                });
                state.dragging = false;
                state.goal_x = None;
                state.restart_blink();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.image_drag.is_some() => {
                let slot = state.drop_slot(to_content(*position).y);
                if let Some(drag) = &mut state.image_drag {
                    drag.position = *position;
                    if drag.start.distance(*position) > DRAG_THRESHOLD {
                        drag.moving = true;
                    }
                    if drag.moving {
                        drag.drop_at = Some(slot);
                    }
                }
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.image_drag.is_some() =>
            {
                if let Some(ImageDrag {
                    line,
                    moving: true,
                    drop_at: Some(to),
                    ..
                }) = state.image_drag.take()
                {
                    self.publish(shell, EditorAction::MoveLine { from: line, to });
                }
                shell.request_redraw();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };
                let click = mouse::Click::new(position, mouse::Button::Left, state.last_click);
                state.last_click = Some(click);
                state.dragging = true;
                state.goal_x = None;
                let kind = match click.kind() {
                    mouse::click::Kind::Single => ClickKind::Single,
                    mouse::click::Kind::Double => ClickKind::Double,
                    mouse::click::Kind::Triple => ClickKind::Triple,
                };
                let extend = kind == ClickKind::Single && state.modifiers.shift();
                let index = state.index_at(self.editor, to_content(position));
                self.publish(
                    shell,
                    EditorAction::Click {
                        index,
                        kind,
                        extend,
                    },
                );
                state.restart_blink();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) if state.dragging => {
                let index = state.index_at(self.editor, to_content(*position));
                if index != self.editor.selection().head {
                    self.publish(shell, EditorAction::Drag { index });
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let Some(position) = cursor.position_over(bounds) else {
                    return;
                };
                let index = state.index_at(self.editor, to_content(position));
                self.publish(shell, EditorAction::ContextMenu { index, position });
                shell.capture_event();
            }
            // ⌘ + rueda (Ctrl fuera de macOS) amplía o reduce la letra.
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(bounds) && state.modifiers.command() =>
            {
                let steps = super::zoom_steps(&mut state.zoom_pending, *delta);
                if steps != 0 {
                    self.publish(shell, EditorAction::Zoom(steps));
                }
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => -y * self.metrics.font_size * 3.0,
                    mouse::ScrollDelta::Pixels { y, .. } => -y,
                };
                self.publish(shell, EditorAction::Scroll { dx: 0.0, dy });
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let colors = self.colors;
        fill(renderer, bounds, colors.background);
        let origin = self.origin(bounds);
        let scroll = self.view.scroll_y;
        let first = state.line_at(scroll - PADDING);
        let last = state.line_at(scroll + bounds.height);
        let buffer = self.editor.buffer();
        let selections = self.editor.selections();

        renderer.with_layer(bounds, |renderer| {
            for line in first..=last {
                let (Some(layout), Some(top)) = (state.lines.get(line), state.tops.get(line))
                else {
                    continue;
                };
                let line_start = buffer.line_start(line);
                let line_len = layout.text.chars().count();
                // Selecciones que tocan la línea.
                for selection in selections {
                    let range = selection.range();
                    if range.is_empty()
                        || range.end < line_start
                        || range.start > line_start + line_len
                    {
                        continue;
                    }
                    let from = range.start.saturating_sub(line_start).min(line_len);
                    let to = (range.end - line_start).min(line_len);
                    let b0 = layout.col_byte.get(from).copied().unwrap_or(0);
                    let b1 = layout.col_byte.get(to).copied().unwrap_or(0);
                    let continues = range.end > line_start + line_len;
                    for run in layout.paragraph.buffer().layout_runs() {
                        let highlight = run.highlight(
                            cosmic_text::Cursor::new(0, b0),
                            cosmic_text::Cursor::new(0, b1),
                        );
                        let (x, mut w) = match highlight {
                            Some(found) => found,
                            None if b0 == b1 && continues => (run.line_w, 0.0),
                            None => continue,
                        };
                        if continues {
                            w += 6.0;
                        }
                        fill(
                            renderer,
                            Rectangle {
                                x: origin.x + x,
                                y: origin.y + top + layout.text_top + run.line_top,
                                width: w,
                                height: run.line_height,
                            },
                            colors.selection,
                        );
                    }
                }
                let resizing = state.image_resize.as_ref().is_some_and(|r| r.line == line);
                if let Some(image) = layout.image.as_ref().filter(|_| !resizing) {
                    renderer.draw_image(
                        advanced_image::Image::new(image.allocation.handle().clone()),
                        Rectangle::new(Point::new(origin.x, origin.y + top), image.size),
                        bounds,
                    );
                }
                let at = Point::new(origin.x, origin.y + top + layout.text_top);
                renderer.fill_paragraph(&layout.paragraph, at, colors.text, bounds);
                // Tachado (por la mitad) y subrayado de los vínculos (abajo).
                for &(span, strike, color) in &layout.lines_on {
                    for rect in layout.paragraph.span_bounds(span) {
                        let y = if strike {
                            rect.y + rect.height * 0.55
                        } else {
                            rect.y + rect.height * 0.85
                        };
                        fill(
                            renderer,
                            Rectangle {
                                x: at.x + rect.x,
                                y: (at.y + y).round(),
                                width: rect.width,
                                height: 1.0,
                            },
                            color.unwrap_or(colors.text),
                        );
                    }
                }
            }

            if self.focused && state.window_focused && state.cursor_visible() {
                for selection in selections {
                    let position = buffer.char_to_position(selection.head);
                    if let Some(caret) = state.caret(position.line, position.column) {
                        fill(
                            renderer,
                            Rectangle {
                                x: (origin.x + caret.x).floor(),
                                y: origin.y + caret.y,
                                ..caret
                            },
                            colors.cursor,
                        );
                    }
                }
            }
        });

        // Marco de la imagen seleccionada y, al arrastrar, la raya del destino y la copia
        // translúcida. En otra capa: dentro de la de antes, las imágenes tapan los quads.
        renderer.with_layer(bounds, |renderer| {
            for line in first..=last {
                let Some(rect) = state.image_rect(line) else {
                    continue;
                };
                let resize = state.image_resize.as_ref().filter(|r| r.line == line);
                if !self.is_image_selected(state, line) && resize.is_none() {
                    continue;
                }
                let mut rect = Rectangle {
                    x: origin.x + rect.x,
                    y: origin.y + rect.y,
                    ..rect
                };
                if let (Some(resize), Some(image)) =
                    (resize, state.lines.get(line).and_then(|l| l.image.as_ref()))
                {
                    // Vista previa del tamaño nuevo, con la misma proporción.
                    rect.height = image.size.height * resize.width / image.size.width.max(1.0);
                    rect.width = resize.width;
                    renderer.draw_image(
                        advanced_image::Image::new(image.allocation.handle().clone()),
                        rect,
                        bounds,
                    );
                }
                outline(renderer, rect, colors.cursor);
                let handle = handle_rect(rect);
                fill(renderer, handle.expand(1.0), colors.background);
                fill(renderer, handle, colors.cursor);
            }
            if let Some(drag) = state.image_drag.as_ref().filter(|drag| drag.moving) {
                if let Some(y) = drag.drop_at.and_then(|slot| state.tops.get(slot)) {
                    fill(
                        renderer,
                        Rectangle {
                            x: origin.x,
                            y: (origin.y + y - 1.0).round(),
                            width: Self::text_width(bounds),
                            height: 2.0,
                        },
                        colors.cursor,
                    );
                }
                if let Some(image) = state.lines.get(drag.line).and_then(|l| l.image.as_ref()) {
                    let scale = (GHOST_WIDTH / image.size.width.max(1.0)).min(1.0);
                    let size = Size::new(image.size.width * scale, image.size.height * scale);
                    renderer.draw_image(
                        advanced_image::Image::new(image.allocation.handle().clone())
                            .opacity(0.5_f32),
                        Rectangle::new(
                            Point::new(drag.position.x + 8.0, drag.position.y + 8.0),
                            size,
                        ),
                        bounds,
                    );
                }
            }
        });

        // Indicador de desplazamiento.
        let total = state.total_height() + 2.0 * PADDING;
        if total > bounds.height && total > 0.0 {
            let length = (bounds.height * bounds.height / total).max(24.0);
            let progress = (scroll / (total - bounds.height).max(1.0)).clamp(0.0, 1.0);
            renderer.with_layer(bounds, |renderer| {
                renderer::Renderer::fill_quad(
                    renderer,
                    Quad {
                        bounds: Rectangle {
                            x: bounds.x + bounds.width - SCROLLBAR + 1.0,
                            y: bounds.y + progress * (bounds.height - length),
                            width: SCROLLBAR - 3.0,
                            height: length,
                        },
                        border: iced::Border {
                            radius: 3.0.into(),
                            ..iced::Border::default()
                        },
                        ..Quad::default()
                    },
                    Background::Color(colors.scrollbar),
                );
            });
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if state.image_drag.as_ref().is_some_and(|drag| drag.moving) {
            return mouse::Interaction::Grabbing;
        }
        if state.image_resize.is_some() {
            return mouse::Interaction::ResizingDiagonallyDown;
        }
        let bounds = layout.bounds();
        let origin = self.origin(bounds);
        if let Some(position) = cursor.position_over(bounds)
            && self
                .handle_at(
                    state,
                    Point::new(position.x - origin.x, position.y - origin.y),
                )
                .is_some()
        {
            return mouse::Interaction::ResizingDiagonallyDown;
        }
        if let Some(position) = cursor.position_over(bounds)
            && state
                .image_at(Point::new(position.x - origin.x, position.y - origin.y))
                .is_some()
        {
            return mouse::Interaction::Grab;
        }
        if cursor.is_over(bounds) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message: 'a> From<RichEditor<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(editor: RichEditor<'a, Message>) -> Self {
        Element::new(editor)
    }
}

/// Tirador de cambio de tamaño de una imagen que ocupa `rect` (esquina inferior derecha).
fn handle_rect(rect: Rectangle) -> Rectangle {
    Rectangle {
        x: rect.x + rect.width - HANDLE / 2.0,
        y: rect.y + rect.height - HANDLE / 2.0,
        width: HANDLE,
        height: HANDLE,
    }
}

/// Marco de 2 px alrededor de `rect` (por fuera).
fn outline(renderer: &mut Renderer, rect: Rectangle, color: Color) {
    let w = 2.0;
    let (x0, y0) = (rect.x - w, rect.y - w);
    let (x1, y1) = (rect.x + rect.width, rect.y + rect.height);
    let width = rect.width + 2.0 * w;
    let height = rect.height + 2.0 * w;
    fill(
        renderer,
        Rectangle {
            x: x0,
            y: y0,
            width,
            height: w,
        },
        color,
    );
    fill(
        renderer,
        Rectangle {
            x: x0,
            y: y1,
            width,
            height: w,
        },
        color,
    );
    fill(
        renderer,
        Rectangle {
            x: x0,
            y: y0,
            width: w,
            height,
        },
        color,
    );
    fill(
        renderer,
        Rectangle {
            x: x1,
            y: y0,
            width: w,
            height,
        },
        color,
    );
}

fn fill(renderer: &mut Renderer, bounds: Rectangle, color: Color) {
    renderer::Renderer::fill_quad(
        renderer,
        Quad {
            bounds,
            ..Quad::default()
        },
        Background::Color(color),
    );
}
