//! Estado de vista de un documento (desplazamiento), separado del modelo.

use lorca_core::{Buffer, EditorState, display};

use super::{LINE_HEIGHT, TAB_WIDTH};

/// Dimensiones del área de texto que el widget comunica a la aplicación.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Viewport {
    /// Anchura útil para el texto (sin el canal de números de línea).
    pub width: f32,
    pub height: f32,
    /// Anchura de una celda de la fuente monoespaciada.
    pub cell_width: f32,
}

/// Vista de un documento: desplazamiento (en píxeles) y últimas medidas conocidas.
///
/// Cada documento tiene la suya porque, con la pantalla dividida, cada panel mide distinto.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DocView {
    pub scroll_x: f32,
    pub scroll_y: f32,
    pub viewport: Viewport,
}

impl DocView {
    /// Desplaza la vista y la mantiene dentro de los límites del documento.
    pub fn scroll_by(&mut self, dx: f32, dy: f32, viewport: Viewport, buffer: &Buffer) {
        self.scroll_x += dx;
        self.scroll_y += dy;
        self.clamp(viewport, buffer);
    }

    /// Limita el desplazamiento: verticalmente hasta que la última línea quede arriba;
    /// horizontalmente hasta el final de la línea más larga.
    pub fn clamp(&mut self, viewport: Viewport, buffer: &Buffer) {
        let max_y = buffer.len_lines().saturating_sub(1) as f32 * LINE_HEIGHT;
        self.scroll_y = self.scroll_y.clamp(0.0, max_y);

        let longest = (0..buffer.len_lines())
            .map(|line| buffer.line_len(line))
            .max()
            .unwrap_or(0);
        let content_width = longest as f32 * viewport.cell_width;
        let max_x = (content_width - viewport.width * 0.5).max(0.0);
        self.scroll_x = self.scroll_x.clamp(0.0, max_x);
    }

    /// `true` si el cursor cae dentro de la vista con estas dimensiones.
    pub fn is_cursor_visible(&self, editor: &EditorState, viewport: Viewport) -> bool {
        if viewport.height <= 0.0 {
            return false;
        }
        let top = editor.cursor().line as f32 * LINE_HEIGHT;
        top >= self.scroll_y && top + LINE_HEIGHT <= self.scroll_y + viewport.height
    }

    /// Para saltos (buscar, ir a línea…): si el cursor no está a la vista, lo coloca a un
    /// tercio de la altura para que se vea el contexto; si ya lo está, no mueve nada.
    pub fn jump_to_cursor(&mut self, editor: &EditorState, viewport: Viewport) {
        if viewport.height <= 0.0 {
            return;
        }
        if !self.is_cursor_visible(editor, viewport) {
            let top = editor.cursor().line as f32 * LINE_HEIGHT;
            self.scroll_y = (top - viewport.height / 3.0).max(0.0);
        }
        self.reveal_cursor(editor, viewport);
    }

    /// Ajusta el desplazamiento mínimo necesario para que el cursor sea visible.
    pub fn reveal_cursor(&mut self, editor: &EditorState, viewport: Viewport) {
        if viewport.height <= 0.0 || viewport.width <= 0.0 {
            return;
        }
        let position = editor.cursor();

        let top = position.line as f32 * LINE_HEIGHT;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if top + LINE_HEIGHT > self.scroll_y + viewport.height {
            self.scroll_y = (top + LINE_HEIGHT - viewport.height).max(0.0);
        }

        let line = editor.buffer().line_text(position.line);
        let cells = display::visual_column(&line, position.column, TAB_WIDTH);
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
    };

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
        let buffer = Buffer::from_text("a\nb\nc");
        let mut view = DocView::default();
        view.scroll_by(-50.0, -50.0, VIEWPORT, &buffer);
        assert_eq!(view, DocView::default());
        view.scroll_by(0.0, 1e6, VIEWPORT, &buffer);
        assert_eq!(view.scroll_y, 2.0 * LINE_HEIGHT);
        view.scroll_by(1e6, 0.0, VIEWPORT, &buffer);
        assert_eq!(view.scroll_x, 0.0);
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
}
