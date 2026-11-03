//! Estado de vista de un documento (desplazamiento), separado del modelo.

use lorca_core::{Buffer, EditorState, Rows, WrapLayout};

use super::Metrics;

/// Dimensiones del área de texto que el widget comunica a la aplicación.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Viewport {
    /// Anchura útil para el texto (sin el canal de números de línea).
    pub width: f32,
    pub height: f32,
    /// Anchura de una celda de la fuente monoespaciada.
    pub cell_width: f32,
    pub metrics: Metrics,
}

/// Ajuste de línea de un documento: si está activo y las filas calculadas para la
/// última versión y anchura conocidas.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WrapCache {
    pub enabled: bool,
    /// Versión del documento y filas calculadas para ella.
    layout: Option<(u64, WrapLayout)>,
}

impl WrapCache {
    /// Recalcula las filas si cambió el documento, la anchura o el tabulador. Sin ajuste
    /// (o sin medidas todavía) las descarta.
    pub fn refresh(&mut self, editor: &EditorState, viewport: Viewport) {
        if !self.enabled || viewport.width <= 0.0 || viewport.cell_width <= 0.0 {
            self.layout = None;
            return;
        }
        let width = ((viewport.width / viewport.cell_width).floor() as usize).max(1);
        let tab_width = viewport.metrics.tab_width;
        let version = editor.version();
        let fresh = self.layout.as_ref().is_some_and(|(cached, layout)| {
            *cached == version && layout.width() == width && layout.tab_width() == tab_width
        });
        if !fresh {
            let layout = WrapLayout::new(editor.buffer(), width, tab_width);
            self.layout = Some((version, layout));
        }
    }

    pub fn layout(&self) -> Option<&WrapLayout> {
        self.layout.as_ref().map(|(_, layout)| layout)
    }
}

/// Vista de un documento: desplazamiento (en píxeles), últimas medidas conocidas y ajuste
/// de línea.
///
/// Cada documento tiene la suya porque, con la pantalla dividida, cada panel mide distinto.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocView {
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub viewport: Viewport,
    pub wrap: WrapCache,
    /// Celda que mantienen ↑/↓ al pasar por filas más cortas (con ajuste de línea).
    pub goal_cells: Option<usize>,
}

/// Longitud (en caracteres) de la línea más larga del documento.
pub fn longest_line(buffer: &Buffer) -> usize {
    (0..buffer.len_lines())
        .map(|line| buffer.line_len(line))
        .max()
        .unwrap_or(0)
}

/// Desplazamiento máximo (horizontal, vertical) para un documento de `rows` filas cuya
/// fila más larga tiene `longest` caracteres.
pub fn max_scroll(viewport: Viewport, rows: usize, longest: usize) -> (f32, f32) {
    let max_y = rows.saturating_sub(1) as f32 * viewport.metrics.line_height;
    let content_width = longest as f32 * viewport.cell_width;
    let max_x = (content_width - viewport.width * 0.5).max(0.0);
    (max_x, max_y)
}

impl DocView {
    /// Filas visuales del documento (con el ajuste de línea calculado, si lo hay).
    pub fn rows<'a>(&'a self, editor: &'a EditorState) -> Rows<'a> {
        Rows::new(
            editor.buffer(),
            self.wrap.layout(),
            self.viewport.metrics.tab_width,
        )
    }

    /// Desplaza la vista y la mantiene dentro de los límites del documento.
    pub fn scroll_by(&mut self, dx: f32, dy: f32, viewport: Viewport, editor: &EditorState) {
        self.scroll_x += dx;
        self.scroll_y += dy;
        self.clamp(viewport, editor);
    }

    /// Lleva la vista a una posición absoluta en los ejes indicados (barras de
    /// desplazamiento) y la mantiene dentro de los límites.
    pub fn scroll_to(
        &mut self,
        x: Option<f32>,
        y: Option<f32>,
        viewport: Viewport,
        editor: &EditorState,
    ) {
        if let Some(x) = x {
            self.scroll_x = x;
        }
        if let Some(y) = y {
            self.scroll_y = y;
        }
        self.clamp(viewport, editor);
    }

    /// Limita el desplazamiento: verticalmente hasta que la última fila quede arriba;
    /// horizontalmente hasta el final de la línea más larga (nada con ajuste de línea).
    pub fn clamp(&mut self, viewport: Viewport, editor: &EditorState) {
        self.wrap.refresh(editor, viewport);
        let rows = self.rows(editor);
        let longest = if rows.is_wrapped() {
            0
        } else {
            longest_line(editor.buffer())
        };
        let (max_x, max_y) = max_scroll(viewport, rows.count(), longest);
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
    }

    /// `true` si el cursor cae dentro de la vista con estas dimensiones.
    pub fn is_cursor_visible(&self, editor: &EditorState, viewport: Viewport) -> bool {
        if viewport.height <= 0.0 {
            return false;
        }
        let (row, _) = self.rows(editor).locate(editor.selection().head);
        let top = row as f32 * viewport.metrics.line_height;
        top >= self.scroll_y
            && top + viewport.metrics.line_height <= self.scroll_y + viewport.height
    }

    /// Para saltos (buscar, ir a línea…): si el cursor no está a la vista, lo coloca a un
    /// tercio de la altura para que se vea el contexto; si ya lo está, no mueve nada.
    pub fn jump_to_cursor(&mut self, editor: &EditorState, viewport: Viewport) {
        if viewport.height <= 0.0 {
            return;
        }
        self.wrap.refresh(editor, viewport);
        if !self.is_cursor_visible(editor, viewport) {
            let (row, _) = self.rows(editor).locate(editor.selection().head);
            let top = row as f32 * viewport.metrics.line_height;
            self.scroll_y = (top - viewport.height / 3.0).max(0.0);
        }
        self.reveal_cursor(editor, viewport);
    }

    /// Ajusta el desplazamiento mínimo necesario para que el cursor sea visible.
    pub fn reveal_cursor(&mut self, editor: &EditorState, viewport: Viewport) {
        if viewport.height <= 0.0 || viewport.width <= 0.0 {
            return;
        }
        self.wrap.refresh(editor, viewport);
        let rows = self.rows(editor);
        let (row, cells) = rows.locate(editor.selection().head);
        let wrapped = rows.is_wrapped();

        let top = row as f32 * viewport.metrics.line_height;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if top + viewport.metrics.line_height > self.scroll_y + viewport.height {
            self.scroll_y = (top + viewport.metrics.line_height - viewport.height).max(0.0);
        }

        if wrapped {
            self.scroll_x = 0.0;
            return;
        }
        let x = cells as f32 * viewport.cell_width;
        let margin = viewport.cell_width * 4.0;
        if x < self.scroll_x {
            self.scroll_x = (x - margin).max(0.0);
        } else if x + margin > self.scroll_x + viewport.width {
            self.scroll_x = x + margin - viewport.width;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_core::Motion;

    const VIEWPORT: Viewport = Viewport {
        width: 80.0,
        height: 100.0,
        cell_width: 8.0,
        metrics: Metrics {
            font_size: 14.0,
            line_height: LINE_HEIGHT,
            tab_width: 4,
        },
    };
    const LINE_HEIGHT: f32 = 20.0;

    #[test]
    fn reveal_scrolls_down_and_up() {
        let mut editor = EditorState::from_text(&"línea\n".repeat(50));
        let mut view = DocView::default();
        editor.move_cursor(Motion::PageDown(20), false);
        view.reveal_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_y, 21.0 * LINE_HEIGHT - VIEWPORT.height);
        editor.move_cursor(Motion::DocumentStart, false);
        view.reveal_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_y, 0.0);
    }

    #[test]
    fn reveal_scrolls_horizontally() {
        let mut editor = EditorState::from_text(&"a".repeat(100));
        let mut view = DocView::default();
        editor.move_cursor(Motion::LineEnd, false);
        view.reveal_cursor(&editor, VIEWPORT);
        assert!(view.scroll_x > 0.0);
        editor.move_cursor(Motion::LineStart, false);
        view.reveal_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_x, 0.0);
    }

    #[test]
    fn scroll_is_clamped() {
        let buffer = EditorState::from_text("a\nb\nc");
        let mut view = DocView::default();
        view.scroll_by(-50.0, -50.0, VIEWPORT, &buffer);
        assert_eq!(view, DocView::default());
        view.scroll_by(0.0, 1e6, VIEWPORT, &buffer);
        assert_eq!(view.scroll_y, 2.0 * LINE_HEIGHT);
        view.scroll_by(1e6, 0.0, VIEWPORT, &buffer);
        assert_eq!(view.scroll_x, 0.0);
    }

    #[test]
    fn scroll_to_sets_only_the_given_axes() {
        let buffer = EditorState::from_text(&format!("{}\n", "a".repeat(100)).repeat(10));
        let mut view = DocView::default();
        view.scroll_to(Some(40.0), None, VIEWPORT, &buffer);
        assert_eq!((view.scroll_x, view.scroll_y), (40.0, 0.0));
        view.scroll_to(None, Some(1e6), VIEWPORT, &buffer);
        assert_eq!(view.scroll_x, 40.0);
        assert_eq!(view.scroll_y, 10.0 * LINE_HEIGHT);
        // Límite horizontal: final de la línea más larga menos media vista.
        assert_eq!(
            max_scroll(VIEWPORT, 11, 100).0,
            100.0 * VIEWPORT.cell_width - VIEWPORT.width * 0.5
        );
    }

    #[test]
    fn jump_places_target_with_context() {
        let mut editor = EditorState::from_text(&"x\n".repeat(200));
        let mut view = DocView::default();
        editor.move_cursor(Motion::PageDown(100), false);
        view.jump_to_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_y, 100.0 * LINE_HEIGHT - VIEWPORT.height / 3.0);
        // Si ya está a la vista, no se mueve.
        editor.move_cursor(Motion::Down, false);
        view.jump_to_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_y, 100.0 * LINE_HEIGHT - VIEWPORT.height / 3.0);
    }

    #[test]
    fn cursor_visibility() {
        let mut editor = EditorState::from_text(&"x\n".repeat(50));
        let view = DocView::default();
        assert!(view.is_cursor_visible(&editor, VIEWPORT));
        assert!(!view.is_cursor_visible(&editor, Viewport::default()));
        editor.move_cursor(Motion::PageDown(10), false);
        assert!(!view.is_cursor_visible(&editor, VIEWPORT));
    }

    #[test]
    fn reveal_ignores_unknown_viewport() {
        let editor = EditorState::from_text("x");
        let mut view = DocView {
            scroll_x: 5.0,
            scroll_y: 5.0,
            ..DocView::default()
        };
        view.reveal_cursor(&editor, Viewport::default());
        assert_eq!(
            view,
            DocView {
                scroll_x: 5.0,
                scroll_y: 5.0,
                ..DocView::default()
            }
        );
    }

    fn wrapped_view() -> (EditorState, DocView) {
        // 80 px / 8 px = 10 celdas por fila.
        let editor = EditorState::from_text(&"palabra ".repeat(30));
        let mut view = DocView {
            viewport: VIEWPORT,
            ..DocView::default()
        };
        view.wrap.enabled = true;
        view.wrap.refresh(&editor, VIEWPORT);
        (editor, view)
    }

    #[test]
    fn wrapped_rows_scroll_vertically_only() {
        let (mut editor, mut view) = wrapped_view();
        assert_eq!(view.rows(&editor).count(), 30);
        editor.move_cursor(Motion::DocumentEnd, false);
        view.reveal_cursor(&editor, VIEWPORT);
        assert_eq!(view.scroll_x, 0.0);
        assert_eq!(view.scroll_y, 30.0 * LINE_HEIGHT - VIEWPORT.height);
        view.scroll_by(1e6, 1e6, VIEWPORT, &editor);
        assert_eq!(view.scroll_x, 0.0);
        assert_eq!(view.scroll_y, 29.0 * LINE_HEIGHT);
    }

    #[test]
    fn wrap_follows_edits_and_can_be_disabled() {
        let (mut editor, mut view) = wrapped_view();
        editor.select_all();
        editor.insert_text("corto").unwrap();
        view.reveal_cursor(&editor, VIEWPORT);
        assert_eq!(view.rows(&editor).count(), 1);
        view.wrap.enabled = false;
        view.wrap.refresh(&editor, VIEWPORT);
        assert!(!view.rows(&editor).is_wrapped());
    }
}
