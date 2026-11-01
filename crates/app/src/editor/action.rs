//! Acciones que el widget pide a la aplicación y su aplicación sobre el núcleo.

use iced::Point;
use lorca_core::{CoreError, EditorState, Motion};

use super::{DocView, INDENT, Viewport};

/// Tipo de clic según cuántos se han encadenado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    Single,
    Double,
    Triple,
}

/// Orden generada por el widget de edición.
#[derive(Debug, Clone, PartialEq)]
pub enum EditorAction {
    Move {
        motion: Motion,
        extend: bool,
    },
    Click {
        index: usize,
        kind: ClickKind,
        extend: bool,
    },
    Drag {
        index: usize,
    },
    Insert(String),
    Newline,
    Indent,
    Backspace,
    Delete,
    DeleteWordBackward,
    DeleteWordForward,
    DeleteToLineStart,
    DeleteSelection,
    SelectAll,
    CollapseSelection,
    Scroll {
        dx: f32,
        dy: f32,
    },
    Viewport(Viewport),
    /// Clic derecho en `index`; `position` es el punto de la ventana donde abrir el menú.
    ContextMenu {
        index: usize,
        position: Point,
    },
}

/// Aplica una acción al documento y a su vista. Tras mover el cursor o editar, desplaza
/// la vista para que el cursor quede visible.
pub fn apply_action(
    action: EditorAction,
    editor: &mut EditorState,
    view: &mut DocView,
    viewport: Viewport,
) -> Result<(), CoreError> {
    match action {
        EditorAction::Scroll { dx, dy } => {
            view.scroll_by(dx, dy, viewport, editor.buffer());
            return Ok(());
        }
        // La aplicación guarda el viewport; aquí solo se reajusta el desplazamiento.
        EditorAction::Viewport(viewport) => {
            view.clamp(viewport, editor.buffer());
            return Ok(());
        }
        // Como en Notepad++: si el clic derecho cae fuera de la selección, mueve el cursor.
        EditorAction::ContextMenu { index, .. } => {
            let selection = editor.selection();
            if selection.is_empty() || !(selection.start()..=selection.end()).contains(&index) {
                editor.set_cursor(index, false);
            }
            return Ok(());
        }
        EditorAction::Move { motion, extend } => editor.move_cursor(motion, extend),
        EditorAction::Click {
            index,
            kind,
            extend,
        } => match kind {
            ClickKind::Single => editor.set_cursor(index, extend),
            ClickKind::Double => editor.select_word_at(index),
            ClickKind::Triple => editor.select_line(editor.buffer().char_to_line(index)),
        },
        EditorAction::Drag { index } => editor.set_cursor(index, true),
        EditorAction::Insert(text) => editor.insert_text(&text)?,
        EditorAction::Newline => editor.insert_newline()?,
        EditorAction::Indent => editor.insert_text(INDENT)?,
        EditorAction::Backspace => editor.backspace()?,
        EditorAction::Delete => editor.delete_forward()?,
        EditorAction::DeleteWordBackward => editor.delete_word_backward()?,
        EditorAction::DeleteWordForward => editor.delete_word_forward()?,
        EditorAction::DeleteToLineStart => {
            if editor.selection().is_empty() {
                editor.move_cursor(Motion::LineStart, true);
            }
            editor.delete_selection()?;
        }
        EditorAction::DeleteSelection => editor.delete_selection()?,
        EditorAction::SelectAll => editor.select_all(),
        EditorAction::CollapseSelection => {
            let head = editor.selection().head;
            editor.set_cursor(head, false);
        }
    }
    view.reveal_cursor(editor, viewport);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Viewport = Viewport {
        width: 400.0,
        height: 200.0,
        cell_width: 8.0,
    };

    fn run(editor: &mut EditorState, actions: Vec<EditorAction>) -> DocView {
        let mut view = DocView::default();
        for action in actions {
            apply_action(action, editor, &mut view, VIEWPORT).unwrap();
        }
        view
    }

    #[test]
    fn typing_and_indent() {
        let mut e = EditorState::new();
        run(
            &mut e,
            vec![
                EditorAction::Indent,
                EditorAction::Insert("si".into()),
                EditorAction::Newline,
                EditorAction::Insert("no".into()),
            ],
        );
        assert_eq!(e.text(), "    si\n    no");
    }

    #[test]
    fn click_kinds() {
        let mut e = EditorState::from_text("bodas de sangre\nyerma");
        run(
            &mut e,
            vec![EditorAction::Click {
                index: 7,
                kind: ClickKind::Double,
                extend: false,
            }],
        );
        assert_eq!(e.selected_text().as_deref(), Some("de"));
        run(
            &mut e,
            vec![EditorAction::Click {
                index: 2,
                kind: ClickKind::Triple,
                extend: false,
            }],
        );
        assert_eq!(e.selected_text().as_deref(), Some("bodas de sangre\n"));
        run(
            &mut e,
            vec![
                EditorAction::Click {
                    index: 0,
                    kind: ClickKind::Single,
                    extend: false,
                },
                EditorAction::Drag { index: 5 },
            ],
        );
        assert_eq!(e.selected_text().as_deref(), Some("bodas"));
    }

    #[test]
    fn delete_to_line_start() {
        let mut e = EditorState::from_text("    texto");
        e.set_cursor(9, false);
        run(&mut e, vec![EditorAction::DeleteToLineStart]);
        assert_eq!(e.text(), "    ");
    }

    #[test]
    fn moving_down_scrolls_to_reveal_cursor() {
        let text = "x\n".repeat(100);
        let mut e = EditorState::from_text(&text);
        let view = run(
            &mut e,
            vec![EditorAction::Move {
                motion: Motion::DocumentEnd,
                extend: false,
            }],
        );
        assert!(view.scroll_y > 0.0);
    }

    #[test]
    fn context_menu_moves_cursor_only_outside_selection() {
        let mut e = EditorState::from_text("bodas de sangre");
        e.set_selection(lorca_core::Selection::new(6, 8));
        let position = Point::ORIGIN;
        run(
            &mut e,
            vec![EditorAction::ContextMenu { index: 7, position }],
        );
        assert_eq!(e.selected_text().as_deref(), Some("de"));
        run(
            &mut e,
            vec![EditorAction::ContextMenu { index: 2, position }],
        );
        assert_eq!(e.selection(), lorca_core::Selection::cursor(2));
    }

    #[test]
    fn collapse_keeps_head() {
        let mut e = EditorState::from_text("abcdef");
        e.set_selection(lorca_core::Selection::new(1, 4));
        run(&mut e, vec![EditorAction::CollapseSelection]);
        assert_eq!(e.selection(), lorca_core::Selection::cursor(4));
    }
}
