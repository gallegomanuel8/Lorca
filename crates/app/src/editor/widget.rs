//! Widget de edición de código: dibuja el buffer y traduce eventos en acciones.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad, Renderer as _};
use iced::advanced::text::{self, Paragraph as _, Renderer as _, Text};
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
use std::ops::Range;

use lorca_core::{EditorState, LineEnding, Position, display};
use lorca_syntax::Highlights;

use super::keymap::{KeyCommand, map_key};
use super::palette::EditorColors;
use super::{ClickKind, DocView, EditorAction, Metrics, TEXT_PADDING, Viewport};

/// Periodo de parpadeo del cursor.
const BLINK_MILLIS: u128 = 530;
/// Anchura de la barra de desplazamiento vertical (solo indicador).
const SCROLLBAR_WIDTH: f32 = 6.0;

/// Crea el widget de edición para un documento.
pub fn code_editor<'a, Message>(
    editor: &'a EditorState,
    view: DocView,
    focused: bool,
    highlights: Option<&'a Highlights>,
    decorations: Decorations<'a>,
    options: ViewOptions,
    on_action: impl Fn(EditorAction) -> Message + 'a,
) -> CodeEditor<'a, Message> {
    CodeEditor {
        editor,
        view,
        focused,
        highlights,
        decorations,
        options,
        colors: None,
        on_action: Box::new(on_action),
    }
}

/// Qué elementos invisibles se dibujan.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewOptions {
    /// Espacios (·) y tabuladores (→).
    pub whitespace: bool,
    /// Etiqueta del final de línea del documento al final de cada línea.
    pub line_ending: Option<LineEnding>,
    pub indent_guides: bool,
    pub metrics: Metrics,
}

/// Rangos (en caracteres, ordenados y sin solapes) que se resaltan bajo el texto.
#[derive(Debug, Clone, Copy, Default)]
pub struct Decorations<'a> {
    /// Coincidencias de la búsqueda actual.
    pub matches: &'a [Range<usize>],
    /// Índice en `matches` de la coincidencia seleccionada.
    pub current: Option<usize>,
    /// Coincidencias marcadas con «Marcar todo».
    pub marks: &'a [Range<usize>],
}

/// Widget de edición. Ver el módulo [`crate::editor`].
pub struct CodeEditor<'a, Message> {
    editor: &'a EditorState,
    view: DocView,
    focused: bool,
    /// Resaltado del documento; puede ir una revisión por detrás mientras se recalcula.
    highlights: Option<&'a Highlights>,
    decorations: Decorations<'a>,
    options: ViewOptions,
    /// Colores del tema activo; sin ellos se derivan del tema de iced.
    colors: Option<&'a EditorColors>,
    on_action: Box<dyn Fn(EditorAction) -> Message + 'a>,
}

impl<Message> std::fmt::Debug for CodeEditor<'_, Message> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CodeEditor")
            .field("selection", &self.editor.selection())
            .field("view", &self.view)
            .field("focused", &self.focused)
            .finish_non_exhaustive()
    }
}

/// Estado interno del widget que persiste entre fotogramas.
#[derive(Debug)]
struct State {
    cell_width: f32,
    /// Tamaño de fuente con el que se midió `cell_width`.
    measured_for: f32,
    last_click: Option<mouse::Click>,
    dragging: bool,
    modifiers: keyboard::Modifiers,
    /// Momento desde el que se cuenta el parpadeo (se reinicia al teclear).
    blink_epoch: Instant,
    now: Instant,
    window_focused: bool,
    reported: Option<Viewport>,
    preedit: Option<input_method::Preedit>,
}

impl Default for State {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            cell_width: 0.0,
            measured_for: 0.0,
            last_click: None,
            dragging: false,
            modifiers: keyboard::Modifiers::default(),
            blink_epoch: now,
            now,
            window_focused: true,
            reported: None,
            preedit: None,
        }
    }
}

impl State {
    fn cursor_visible(&self) -> bool {
        let elapsed = self
            .now
            .saturating_duration_since(self.blink_epoch)
            .as_millis();
        (elapsed / BLINK_MILLIS).is_multiple_of(2)
    }

    fn restart_blink(&mut self) {
        self.blink_epoch = Instant::now();
        self.now = self.blink_epoch;
    }
}

/// Rectángulos del widget calculados a partir de sus límites.
struct Geometry {
    bounds: Rectangle,
    gutter: Rectangle,
    text_area: Rectangle,
    cell: f32,
    metrics: Metrics,
}

impl Geometry {
    fn new(bounds: Rectangle, cell: f32, line_count: usize, metrics: Metrics) -> Self {
        let digits = line_count.to_string().len().max(3);
        let gutter_width = (digits as f32 + 2.0) * cell;
        let gutter = Rectangle {
            width: gutter_width.min(bounds.width),
            ..bounds
        };
        let text_area = Rectangle {
            x: bounds.x + gutter.width,
            width: (bounds.width - gutter.width).max(0.0),
            ..bounds
        };
        Self {
            bounds,
            gutter,
            text_area,
            cell,
            metrics,
        }
    }

    fn line_y(&self, line: usize, view: DocView) -> f32 {
        self.bounds.y + line as f32 * self.metrics.line_height - view.scroll_y
    }

    fn cell_x(&self, cells: usize, view: DocView) -> f32 {
        self.text_area.x + TEXT_PADDING + cells as f32 * self.cell - view.scroll_x
    }

    fn viewport(&self) -> Viewport {
        Viewport {
            width: (self.text_area.width - TEXT_PADDING - SCROLLBAR_WIDTH).max(0.0),
            height: self.bounds.height,
            cell_width: self.cell,
            metrics: self.metrics,
        }
    }

    fn page_lines(&self) -> usize {
        ((self.bounds.height / self.metrics.line_height).floor() as usize)
            .saturating_sub(1)
            .max(1)
    }
}

impl<'a, Message> CodeEditor<'a, Message> {
    /// Usa los colores del tema activo.
    pub fn colors(mut self, colors: &'a EditorColors) -> Self {
        self.colors = Some(colors);
        self
    }

    /// Índice de carácter más cercano a un punto de la pantalla.
    fn index_at(&self, point: Point, geometry: &Geometry) -> usize {
        let buffer = self.editor.buffer();
        let y = point.y - geometry.bounds.y + self.view.scroll_y;
        let line = if y <= 0.0 {
            0
        } else {
            (y / geometry.metrics.line_height) as usize
        };
        let line = line.min(buffer.len_lines().saturating_sub(1));
        let cells =
            (point.x - geometry.text_area.x - TEXT_PADDING + self.view.scroll_x) / geometry.cell;
        let column = display::char_column_at_visual(
            &buffer.line_text(line),
            cells.max(0.0),
            geometry.metrics.tab_width,
        );
        buffer.position_to_char(Position::new(line, column))
    }

    /// Rectángulo del cursor principal en pantalla.
    fn cursor_rect(&self, geometry: &Geometry) -> Rectangle {
        self.cursor_rect_at(self.editor.selection().head, geometry)
    }

    /// Rectángulo de un cursor situado en el índice `head`.
    fn cursor_rect_at(&self, head: usize, geometry: &Geometry) -> Rectangle {
        let buffer = self.editor.buffer();
        let position = buffer.char_to_position(head);
        let line = buffer.line_text(position.line);
        let cells = display::visual_column(&line, position.column, geometry.metrics.tab_width);
        Rectangle {
            x: geometry.cell_x(cells, self.view).floor(),
            y: geometry.line_y(position.line, self.view),
            width: 2.0,
            height: geometry.metrics.line_height,
        }
    }

    fn input_method<'b>(&self, state: &'b State, geometry: &Geometry) -> InputMethod<&'b str> {
        if !self.focused || !state.window_focused {
            return InputMethod::Disabled;
        }
        InputMethod::Enabled {
            cursor: self.cursor_rect(geometry),
            purpose: input_method::Purpose::Normal,
            preedit: state.preedit.as_ref().map(input_method::Preedit::as_ref),
        }
    }

    fn publish(&self, shell: &mut Shell<'_, Message>, action: EditorAction) {
        shell.publish((self.on_action)(action));
    }
}

impl<Message> Widget<Message, Theme, Renderer> for CodeEditor<'_, Message> {
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
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        let metrics = self.options.metrics;
        if state.cell_width <= 0.0 || state.measured_for != metrics.font_size {
            state.cell_width = measure_cell_width(metrics);
            state.measured_for = metrics.font_size;
        }
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
        let geometry = Geometry::new(
            layout.bounds(),
            state.cell_width,
            self.editor.buffer().len_lines(),
            self.options.metrics,
        );

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
                let viewport = geometry.viewport();
                if state.reported != Some(viewport) {
                    state.reported = Some(viewport);
                    self.publish(shell, EditorAction::Viewport(viewport));
                }
                if self.focused && state.window_focused {
                    let elapsed = now.saturating_duration_since(state.blink_epoch).as_millis();
                    let wait = BLINK_MILLIS - elapsed % BLINK_MILLIS;
                    let wait = u64::try_from(wait).unwrap_or(1);
                    shell.request_redraw_at(*now + Duration::from_millis(wait));
                }
                shell.request_input_method(&self.input_method(state, &geometry));
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
                let Some(command) = map_key(
                    key,
                    *physical_key,
                    *modifiers,
                    text.as_deref(),
                    geometry.page_lines(),
                ) else {
                    return;
                };
                match command {
                    KeyCommand::Edit(EditorAction::CollapseSelection)
                        if self.editor.selection().is_empty()
                            && self.editor.selections().len() == 1 =>
                    {
                        return;
                    }
                    KeyCommand::Edit(action) => self.publish(shell, action),
                    // Con varios cursores se copian todas las selecciones, una por línea.
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
                        if let Some(contents) = clipboard.read(clipboard::Kind::Standard) {
                            self.publish(shell, EditorAction::Insert(contents));
                        }
                    }
                }
                state.restart_blink();
                shell.capture_event();
            }
            Event::InputMethod(event) if self.focused && !shell.is_event_captured() => {
                match event {
                    input_method::Event::Opened => {
                        state.preedit = Some(input_method::Preedit::new());
                        shell.request_redraw();
                    }
                    input_method::Event::Closed => {
                        state.preedit = None;
                        shell.request_redraw();
                    }
                    input_method::Event::Preedit(content, selection) => {
                        state.preedit = Some(input_method::Preedit {
                            content: content.clone(),
                            selection: selection.clone(),
                            text_size: Some(Pixels(geometry.metrics.font_size)),
                        });
                        shell.request_redraw();
                    }
                    input_method::Event::Commit(content) => {
                        self.publish(shell, EditorAction::Insert(content.clone()));
                        state.restart_blink();
                        shell.capture_event();
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(position) = cursor.position_over(geometry.bounds) else {
                    return;
                };
                let click = mouse::Click::new(position, mouse::Button::Left, state.last_click);
                state.last_click = Some(click);
                state.dragging = true;
                let kind = match click.kind() {
                    mouse::click::Kind::Single => ClickKind::Single,
                    mouse::click::Kind::Double => ClickKind::Double,
                    mouse::click::Kind::Triple => ClickKind::Triple,
                };
                let extend = kind == ClickKind::Single && state.modifiers.shift();
                let index = self.index_at(position, &geometry);
                // ⌥+clic (Alt+clic) añade un cursor.
                if kind == ClickKind::Single && state.modifiers.alt() {
                    state.dragging = false;
                    self.publish(shell, EditorAction::AddCursor { index });
                    state.restart_blink();
                    shell.capture_event();
                    return;
                }
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
                let index = self.index_at(*position, &geometry);
                if index != self.editor.selection().head {
                    self.publish(shell, EditorAction::Drag { index });
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let Some(position) = cursor.position_over(geometry.bounds) else {
                    return;
                };
                let index = self.index_at(position, &geometry);
                self.publish(shell, EditorAction::ContextMenu { index, position });
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(geometry.bounds) =>
            {
                let (mut dx, mut dy) = match delta {
                    mouse::ScrollDelta::Lines { x, y } => (
                        -x * geometry.cell * 3.0,
                        -y * geometry.metrics.line_height * 3.0,
                    ),
                    mouse::ScrollDelta::Pixels { x, y } => (-x, -y),
                };
                // Mayús + rueda desplaza en horizontal (Windows y Linux).
                if state.modifiers.shift() && dx == 0.0 {
                    std::mem::swap(&mut dx, &mut dy);
                }
                self.publish(shell, EditorAction::Scroll { dx, dy });
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let buffer = self.editor.buffer();
        let line_count = buffer.len_lines();
        let geometry = Geometry::new(
            layout.bounds(),
            state.cell_width,
            line_count,
            self.options.metrics,
        );
        let derived;
        let colors = match self.colors {
            Some(colors) => colors,
            None => {
                derived = EditorColors::new(None, theme);
                &derived
            }
        };
        let view = self.view;

        fill(renderer, geometry.text_area, colors.background);
        fill(renderer, geometry.gutter, colors.gutter);

        let first_line = (view.scroll_y / geometry.metrics.line_height)
            .floor()
            .max(0.0) as usize;
        let last_line = (((view.scroll_y + geometry.bounds.height) / geometry.metrics.line_height)
            .ceil() as usize)
            .min(line_count);
        let first_cell = (view.scroll_x / geometry.cell).floor().max(0.0) as usize;
        let visible_cells = (geometry.text_area.width / geometry.cell).ceil() as usize + 2;

        let selection = self.editor.selection();
        let selections = self.editor.selections();
        let cursor_line = buffer.char_to_line(selection.head);

        // Números de línea.
        renderer.with_layer(geometry.gutter, |renderer| {
            for line in first_line..last_line {
                let color = if line == cursor_line {
                    colors.line_number_active
                } else {
                    colors.line_number
                };
                if self.editor.bookmarks().contains(line) {
                    let size = geometry.metrics.line_height * 0.45;
                    renderer::Renderer::fill_quad(
                        renderer,
                        Quad {
                            bounds: Rectangle {
                                x: geometry.gutter.x + geometry.cell * 0.4,
                                y: geometry.line_y(line, view)
                                    + (geometry.metrics.line_height - size) / 2.0,
                                width: size,
                                height: size,
                            },
                            border: iced::Border {
                                radius: (size / 2.0).into(),
                                ..iced::Border::default()
                            },
                            ..Quad::default()
                        },
                        Background::Color(colors.bookmark),
                    );
                }
                renderer.fill_text(
                    Text {
                        content: (line + 1).to_string(),
                        bounds: Size::new(geometry.gutter.width, geometry.metrics.line_height),
                        align_x: text::Alignment::Right,
                        ..text_style(geometry.metrics)
                    },
                    Point::new(
                        geometry.gutter.x + geometry.gutter.width - geometry.cell,
                        geometry.line_y(line, view),
                    ),
                    color,
                    geometry.gutter,
                );
            }
        });

        let guides = if self.options.indent_guides {
            indent_guides(
                self.editor,
                first_line,
                last_line,
                geometry.metrics.tab_width,
            )
        } else {
            Vec::new()
        };

        // Texto, selección y cursor.
        renderer.with_layer(geometry.text_area, |renderer| {
            for line in first_line..last_line {
                let y = geometry.line_y(line, view);
                let text = buffer.line_text(line);
                let line_start = buffer.line_start(line);
                let line_end = line_start + text.chars().count();

                if line == cursor_line && selection.is_empty() && self.focused {
                    fill(
                        renderer,
                        Rectangle {
                            y,
                            height: geometry.metrics.line_height,
                            ..geometry.text_area
                        },
                        colors.current_line,
                    );
                }

                let decorations = self.decorations;
                let line_ranges = LineRanges {
                    text: &text,
                    line_start,
                    line_end,
                    y,
                };
                line_ranges.fill(renderer, &geometry, view, decorations.marks, colors.mark);
                line_ranges.fill(renderer, &geometry, view, decorations.matches, colors.found);
                if let Some(current) = decorations.current.and_then(|i| decorations.matches.get(i))
                {
                    line_ranges.fill(
                        renderer,
                        &geometry,
                        view,
                        std::slice::from_ref(current),
                        colors.found_current,
                    );
                }

                // Selecciones (todas las de los cursores) que tocan esta línea.
                let first = selections.partition_point(|s| s.end() < line_start);
                for selected in selections
                    .iter()
                    .skip(first)
                    .map(lorca_core::Selection::range)
                {
                    if selected.start > line_end {
                        break;
                    }
                    if selected.is_empty()
                        || selected.end <= line_start && selected.start < line_start
                    {
                        continue;
                    }
                    let start = selected.start.max(line_start) - line_start;
                    let end = selected.end.min(line_end) - line_start;
                    let mut end_cells =
                        display::visual_column(&text, end, geometry.metrics.tab_width);
                    // Si la selección incluye el salto de línea, se marca una celda más.
                    if selected.end > line_end {
                        end_cells += 1;
                    }
                    let start_cells =
                        display::visual_column(&text, start, geometry.metrics.tab_width);
                    let x = geometry.cell_x(start_cells, view);
                    fill(
                        renderer,
                        Rectangle {
                            x,
                            y,
                            width: (end_cells.saturating_sub(start_cells)) as f32 * geometry.cell,
                            height: geometry.metrics.line_height,
                        },
                        colors.selection,
                    );
                }

                if let Some(&indent) = guides.get(line - first_line) {
                    for column in (0..indent).step_by(geometry.metrics.tab_width) {
                        fill(
                            renderer,
                            Rectangle {
                                x: geometry.cell_x(column, view).floor(),
                                y,
                                width: 1.0,
                                height: geometry.metrics.line_height,
                            },
                            colors.guide,
                        );
                    }
                }
                if self.options.whitespace {
                    draw_whitespace(
                        renderer,
                        &geometry,
                        view,
                        &text,
                        y,
                        first_cell,
                        visible_cells,
                        colors.whitespace,
                    );
                }
                if let Some(ending) = self.options.line_ending
                    && line + 1 < line_count
                {
                    let cells =
                        display::visual_column(&text, usize::MAX, geometry.metrics.tab_width);
                    let label = ending.label();
                    let x = geometry.cell_x(cells, view) + 4.0;
                    let width = label.len() as f32 * 6.0 + 6.0;
                    renderer::Renderer::fill_quad(
                        renderer,
                        Quad {
                            bounds: Rectangle {
                                x,
                                y: y + 4.0,
                                width,
                                height: geometry.metrics.line_height - 8.0,
                            },
                            border: iced::Border {
                                radius: 3.0.into(),
                                ..iced::Border::default()
                            },
                            ..Quad::default()
                        },
                        Background::Color(colors.guide),
                    );
                    renderer.fill_text(
                        Text {
                            content: label.to_owned(),
                            size: Pixels(9.0),
                            font: Font::default(),
                            align_y: alignment::Vertical::Center,
                            ..text_style(geometry.metrics)
                        },
                        Point::new(x + 3.0, y + geometry.metrics.line_height / 2.0),
                        colors.line_number,
                        geometry.text_area,
                    );
                }

                let spans = self.highlights.map_or(&[][..], |h| h.line(line));
                let mut next_span = 0;
                let runs = display::visible_runs(
                    &text,
                    first_cell,
                    visible_cells,
                    geometry.metrics.tab_width,
                    |index| {
                        while spans.get(next_span).is_some_and(|s| s.end <= index) {
                            next_span += 1;
                        }
                        spans
                            .get(next_span)
                            .filter(|s| s.start <= index)
                            .map(|s| s.kind)
                    },
                );
                for run in runs {
                    if run.text.trim().is_empty() {
                        continue;
                    }
                    let style = colors.style(run.style);
                    renderer.fill_text(
                        Text {
                            content: run.text,
                            font: style.font,
                            ..text_style(geometry.metrics)
                        },
                        Point::new(geometry.cell_x(run.start_cell, view), y),
                        style.color.unwrap_or(colors.text),
                        geometry.text_area,
                    );
                }
            }

            if self.focused && state.window_focused && state.cursor_visible() {
                for selection in self.editor.selections() {
                    fill(
                        renderer,
                        self.cursor_rect_at(selection.head, &geometry),
                        colors.cursor,
                    );
                }
            }

            // Indicador de posición vertical.
            let content_height =
                line_count as f32 * geometry.metrics.line_height + geometry.bounds.height;
            if line_count as f32 * geometry.metrics.line_height > geometry.bounds.height {
                let height = geometry.bounds.height;
                let thumb_height = (height * height / content_height).max(24.0);
                let max_scroll =
                    (line_count.saturating_sub(1)) as f32 * geometry.metrics.line_height;
                let progress = if max_scroll > 0.0 {
                    view.scroll_y / max_scroll
                } else {
                    0.0
                };
                fill(
                    renderer,
                    Rectangle {
                        x: geometry.bounds.x + geometry.bounds.width - SCROLLBAR_WIDTH,
                        y: geometry.bounds.y + progress * (height - thumb_height),
                        width: SCROLLBAR_WIDTH,
                        height: thumb_height,
                    },
                    colors.scrollbar,
                );
            }
        });
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
        let geometry = Geometry::new(
            layout.bounds(),
            state.cell_width,
            self.editor.buffer().len_lines(),
            self.options.metrics,
        );
        if cursor.is_over(geometry.text_area) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }
}

impl<'a, Message: 'a> From<CodeEditor<'a, Message>> for Element<'a, Message, Theme, Renderer> {
    fn from(editor: CodeEditor<'a, Message>) -> Self {
        Element::new(editor)
    }
}

/// Anchura (en celdas) de las guías de indentación de cada línea visible. Las líneas en
/// blanco toman la menor indentación de la línea con texto anterior y la siguiente.
fn indent_guides(editor: &EditorState, first: usize, last: usize, tab_width: usize) -> Vec<usize> {
    /// Hasta dónde se buscan líneas con texto alrededor de una línea en blanco.
    const LOOKAROUND: usize = 200;
    let buffer = editor.buffer();
    let indent_of = |line: usize| leading_indent(&buffer.line_text(line), tab_width);
    (first..last)
        .map(|line| {
            indent_of(line).unwrap_or_else(|| {
                let previous = (line.saturating_sub(LOOKAROUND)..line)
                    .rev()
                    .find_map(&indent_of);
                let next =
                    (line + 1..(line + LOOKAROUND).min(buffer.len_lines())).find_map(&indent_of);
                previous.unwrap_or(0).min(next.unwrap_or(0))
            })
        })
        .collect()
}

/// Celdas de sangría al principio de la línea, o `None` si está en blanco.
fn leading_indent(text: &str, tab_width: usize) -> Option<usize> {
    let mut visual = 0;
    for c in text.chars() {
        match c {
            ' ' | '\t' => visual += display::char_cells(c, visual, tab_width),
            _ => return Some(visual),
        }
    }
    None
}

/// Dibuja un punto en cada espacio y una flecha en cada tabulador de la parte visible.
#[allow(clippy::too_many_arguments)] // Son las coordenadas de la línea; agruparlas no aclara.
fn draw_whitespace(
    renderer: &mut Renderer,
    geometry: &Geometry,
    view: DocView,
    text: &str,
    y: f32,
    first_cell: usize,
    visible_cells: usize,
    color: Color,
) {
    let mut visual = 0;
    for c in text.chars() {
        let width = display::char_cells(c, visual, geometry.metrics.tab_width);
        if visual >= first_cell + visible_cells {
            break;
        }
        if visual + width > first_cell {
            let x = geometry.cell_x(visual, view);
            let middle = y + geometry.metrics.line_height / 2.0;
            match c {
                ' ' => fill(
                    renderer,
                    Rectangle {
                        x: (x + geometry.cell / 2.0 - 1.0).round(),
                        y: (middle - 1.0).round(),
                        width: 2.0,
                        height: 2.0,
                    },
                    color,
                ),
                '\t' => {
                    let end = geometry.cell_x(visual + width, view) - 3.0;
                    let start = x + 2.0;
                    fill(
                        renderer,
                        Rectangle {
                            x: start,
                            y: middle.round(),
                            width: (end - start).max(1.0),
                            height: 1.0,
                        },
                        color,
                    );
                    fill(
                        renderer,
                        Rectangle {
                            x: end - 1.0,
                            y: middle.round() - 3.0,
                            width: 1.0,
                            height: 7.0,
                        },
                        color,
                    );
                }
                _ => {}
            }
        }
        visual += width;
    }
}

/// Una línea visible, para pintar los tramos de rangos que caen en ella.
struct LineRanges<'a> {
    text: &'a str,
    line_start: usize,
    /// Índice del salto de línea (o del final del texto).
    line_end: usize,
    y: f32,
}

impl LineRanges<'_> {
    /// Pinta la parte de cada rango que cae en la línea. Si un rango sigue en la línea
    /// siguiente, se marca también la celda del salto.
    fn fill(
        &self,
        renderer: &mut Renderer,
        geometry: &Geometry,
        view: DocView,
        ranges: &[Range<usize>],
        color: Color,
    ) {
        let first = ranges.partition_point(|r| r.end <= self.line_start);
        for range in ranges.iter().skip(first) {
            if range.start > self.line_end {
                break;
            }
            let start = range.start.max(self.line_start) - self.line_start;
            let end = range.end.min(self.line_end) - self.line_start;
            let start_cells = display::visual_column(self.text, start, geometry.metrics.tab_width);
            let mut end_cells = display::visual_column(self.text, end, geometry.metrics.tab_width);
            if range.end > self.line_end {
                end_cells += 1;
            }
            fill(
                renderer,
                Rectangle {
                    x: geometry.cell_x(start_cells, view),
                    y: self.y,
                    width: end_cells.saturating_sub(start_cells) as f32 * geometry.cell,
                    height: geometry.metrics.line_height,
                },
                color,
            );
        }
    }
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

/// Estilo común del texto del editor (el contenido se rellena en cada uso).
fn text_style(metrics: Metrics) -> Text<String, Font> {
    Text {
        content: String::new(),
        bounds: Size::new(f32::INFINITY, metrics.line_height),
        size: Pixels(metrics.font_size),
        line_height: text::LineHeight::Absolute(Pixels(metrics.line_height)),
        font: Font::MONOSPACE,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    }
}

/// Mide la anchura de una celda de la fuente monoespaciada.
fn measure_cell_width(metrics: Metrics) -> f32 {
    const SAMPLE: &str = "MMMMMMMMMM";
    let style = text_style(metrics);
    let paragraph = <Renderer as text::Renderer>::Paragraph::with_text(Text {
        content: SAMPLE,
        shaping: text::Shaping::Basic,
        ..style.with_content(SAMPLE)
    });
    let width = paragraph.min_bounds().width / SAMPLE.len() as f32;
    if width > 0.0 {
        width
    } else {
        metrics.font_size * 0.6
    }
}
