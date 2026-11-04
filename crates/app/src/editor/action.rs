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
    /// Como `Insert`, pero con texto del portapapeles (puede deducir el lenguaje).
    Paste(String),
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
    /// Posición absoluta en los ejes indicados (arrastre de las barras de desplazamiento).
    /// Absoluta para que varios movimientos seguidos antes de redibujar no se acumulen.
    ScrollTo {
        x: Option<f32>,
        y: Option<f32>,
    },
    Viewport(Viewport),
    /// Alto del contenido del editor con formato (sus líneas miden distinto).
    RichHeight(f32),
    /// ⌥+clic: añade un cursor.
    AddCursor {
        index: usize,
    },
    /// ⌘D: selecciona la palabra o añade la siguiente aparición.
    AddNextOccurrence,
    /// ⌥⌘↑/↓: añade un cursor en la línea de arriba o de abajo.
    AddCursorVertical {
        up: bool,
    },
    /// Clic derecho en `index`; `position` es el punto de la ventana donde abrir el menú.
    ContextMenu {
        index: usize,
        position: Point,
    },
    /// ⌘+clic (Ctrl+clic): ir a la definición de lo que hay en `index`.
    GoToDefinition {
        index: usize,
    },
    /// Punto de la ventana justo debajo del cursor principal (para los desplegables).
    CursorPoint(Point),
    /// Tecla con el desplegable de autocompletado abierto.
    Completion(CompletionKey),
}

/// Teclas que maneja el desplegable de autocompletado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKey {
    Up,
    Down,
    Accept,
}

/// Aplica una acción al documento y a su vista. Tras mover el cursor o editar, desplaza
/// la vista para que el cursor quede visible.
pub fn apply_action(
    action: EditorAction,
    editor: &mut EditorState,
    view: &mut DocView,
    viewport: Viewport,
) -> Result<(), CoreError> {
    // La celda deseada de ↑/↓ solo se conserva entre movimientos verticales seguidos.
    if !matches!(
        action,
        EditorAction::Move {
            motion: Motion::Up | Motion::Down | Motion::PageUp(_) | Motion::PageDown(_),
            ..
        } | EditorAction::Viewport(_)
            | EditorAction::CursorPoint(_)
            | EditorAction::Scroll { .. }
            | EditorAction::ScrollTo { .. }
    ) {
        view.goal_cells = None;
    }
    match action {
        EditorAction::Scroll { dx, dy } => {
            view.scroll_by(dx, dy, viewport, editor);
            return Ok(());
        }
        EditorAction::ScrollTo { x, y } => {
            view.scroll_to(x, y, viewport, editor);
            return Ok(());
        }
        EditorAction::RichHeight(height) => {
            view.rich_height = Some(height);
            view.clamp(viewport, editor);
            return Ok(());
        }
        // La aplicación guarda el viewport; aquí solo se reajusta el desplazamiento.
        EditorAction::Viewport(viewport) => {
            view.clamp(viewport, editor);
            return Ok(());
        }
        // Como en Notepad++: si el clic derecho cae fuera de la selección, mueve el cursor.
        // Las maneja la aplicación.
        EditorAction::CursorPoint(_) | EditorAction::Completion(_) => return Ok(()),
        EditorAction::GoToDefinition { index } => {
            editor.set_cursor(index, false);
            return Ok(());
        }
        EditorAction::ContextMenu { index, .. } => {
            let selection = editor.selection();
            if selection.is_empty() || !(selection.start()..=selection.end()).contains(&index) {
                editor.set_cursor(index, false);
            }
            return Ok(());
        }
        EditorAction::Move { motion, extend } => {
            view.wrap.refresh(editor, viewport);
            if !move_by_rows(motion, extend, editor, view) {
                editor.move_cursor(motion, extend);
            }
        }
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
        EditorAction::AddCursor { index } => editor.add_cursor(index),
        EditorAction::AddNextOccurrence => {
            editor.add_next_occurrence();
        }
        EditorAction::AddCursorVertical { up } => {
            editor.add_cursor_vertical(up);
        }
        // Con tantos cursores como líneas pegadas, se reparte una línea a cada uno.
        EditorAction::Insert(text) | EditorAction::Paste(text) if editor.selections().len() > 1 => {
            let lines: Vec<String> = text
                .trim_end_matches('\n')
                .split('\n')
                .map(str::to_owned)
                .collect();
            if lines.len() == editor.selections().len() {
                editor.insert_texts(&lines)?;
            } else {
                editor.insert_text(&text)?;
            }
        }
        EditorAction::Insert(text) | EditorAction::Paste(text) => editor.insert_text(&text)?,
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
        // Esc: primero deja un solo cursor; después quita la selección.
        EditorAction::CollapseSelection => {
            if editor.selections().len() > 1 {
                editor.collapse_to_primary();
            } else {
                let head = editor.selection().head;
                editor.set_cursor(head, false);
            }
        }
    }
    view.reveal_cursor(editor, viewport);
    Ok(())
}

/// Con ajuste de línea, ↑/↓, Re Pág/Av Pág, Inicio y Fin se mueven por filas visuales (como
/// en el Bloc de notas). Devuelve `false` si no hay ajuste o el movimiento no es de estos.
fn move_by_rows(
    motion: Motion,
    extend: bool,
    editor: &mut EditorState,
    view: &mut DocView,
) -> bool {
    let rows = view.rows(editor);
    if !rows.is_wrapped() {
        return false;
    }
    let single = editor.selections().len() == 1;
    let goal = if single { view.goal_cells } else { None };
    let delta = match motion {
        Motion::Up => Some(-1),
        Motion::Down => Some(1),
        Motion::PageUp(lines) => Some(-isize::try_from(lines).unwrap_or(isize::MAX)),
        Motion::PageDown(lines) => Some(isize::try_from(lines).unwrap_or(isize::MAX)),
        _ => None,
    };
    let heads: Vec<usize> = editor.selections().iter().map(|s| s.head).collect();
    let targets: Vec<usize> = match (motion, delta) {
        (_, Some(delta)) => heads
            .iter()
            .map(|&head| rows.vertical_target(head, delta, goal))
            .collect(),
        (Motion::LineEnd, None) => heads.iter().map(|&head| rows.row_bounds(head).1).collect(),
        // En la primera fila de la línea, Inicio funciona como siempre (salta la sangría).
        (Motion::LineStart, None) => {
            let (row, _) = rows.locate(editor.selection().head);
            if rows.row(row).is_first() {
                return false;
            }
            heads.iter().map(|&head| rows.row_bounds(head).0).collect()
        }
        _ => return false,
    };
    let new_goal = match (delta, single) {
        (Some(_), true) => Some(goal.unwrap_or_else(|| rows.locate(editor.selection().head).1)),
        _ => None,
    };
    let mut targets = targets.into_iter();
    editor.move_cursors_to(extend, |head| targets.next().unwrap_or(head));
    view.goal_cells = new_goal;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Viewport = Viewport {
        width: 400.0,
        height: 200.0,
        cell_width: 8.0,
        metrics: super::super::Metrics {
            font_size: 14.0,
            line_height: 20.0,
            tab_width: 4,
        },
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

    #[test]
    fn wrapped_text_moves_by_rows() {
        // 400 px / 8 px = 50 celdas: «palabra » × 10 son 80 caracteres → 2 filas.
        let mut e = EditorState::from_text(&"palabra ".repeat(10));
        let mut view = DocView {
            viewport: VIEWPORT,
            ..DocView::default()
        };
        view.wrap.enabled = true;
        let down = EditorAction::Move {
            motion: Motion::Down,
            extend: false,
        };
        apply_action(down.clone(), &mut e, &mut view, VIEWPORT).unwrap();
        // Primera fila: 6 palabras (48 caracteres); baja a la celda 0 de la segunda.
        assert_eq!(e.selection().head, 48);
        let end = EditorAction::Move {
            motion: Motion::LineEnd,
            extend: false,
        };
        apply_action(end, &mut e, &mut view, VIEWPORT).unwrap();
        assert_eq!(e.selection().head, 80);
        let up = EditorAction::Move {
            motion: Motion::Up,
            extend: false,
        };
        apply_action(up, &mut e, &mut view, VIEWPORT).unwrap();
        assert_eq!(e.selection().head, 32);
        // Inicio en una fila que no es la primera va al principio de la fila.
        apply_action(down, &mut e, &mut view, VIEWPORT).unwrap();
        let home = EditorAction::Move {
            motion: Motion::LineStart,
            extend: false,
        };
        apply_action(home, &mut e, &mut view, VIEWPORT).unwrap();
        assert_eq!(e.selection().head, 48);
    }
}
