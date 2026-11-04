//! Reglas de edición del modo con formato: como las marcas de Markdown no se ven, el
//! cursor las salta, Retroceso no las rompe y Intro continúa las listas.

use lorca_core::{CoreError, EditorState, Motion, Selection};
use lorca_formats::{ListContinuation, continue_list, parse_line};

use super::EditorAction;

/// Aplica `action` si en el modo con formato necesita un trato especial. Devuelve `None`
/// si debe aplicarse como siempre.
pub fn apply(action: &EditorAction, editor: &mut EditorState) -> Option<Result<(), CoreError>> {
    let single = editor.selections().len() == 1;
    let empty = editor.selection().is_empty();
    match action {
        EditorAction::Move {
            motion: motion @ (Motion::Left | Motion::Right),
            extend,
        } if single => {
            move_visibly(editor, *motion, *extend);
            Some(Ok(()))
        }
        EditorAction::Backspace if single && empty => Some(backspace(editor)),
        EditorAction::Delete if single && empty => {
            let (line, col) = head(editor);
            let markup = parse_line(&editor.buffer().line_text(line));
            if let Some(run) = markup
                .hidden_runs()
                .into_iter()
                .find(|run| run.start == col && run.end < markup.chars.len())
            {
                let start = editor.buffer().line_start(line);
                editor.set_cursor(start + run.end, false);
            }
            None
        }
        EditorAction::Newline if single && empty => newline(editor),
        _ => None,
    }
}

/// Tras cualquier acción, el cursor no queda antes de la marca de bloque ni dentro de una
/// marca oculta.
pub fn snap_cursor(editor: &mut EditorState) {
    if editor.selections().len() != 1 {
        return;
    }
    let selection = editor.selection();
    let (line, col) = head(editor);
    let markup = parse_line(&editor.buffer().line_text(line));
    let snapped = markup.snap(col, true);
    if snapped != col {
        let start = editor.buffer().line_start(line);
        let head = start + snapped;
        let anchor = if selection.is_empty() {
            head
        } else {
            selection.anchor
        };
        editor.set_selection(Selection::new(anchor, head));
    }
}

fn head(editor: &EditorState) -> (usize, usize) {
    let position = editor.buffer().char_to_position(editor.selection().head);
    (position.line, position.column)
}

/// ← / →: se repite el movimiento hasta que el cursor cambie de sitio en pantalla (las
/// marcas ocultas no ocupan sitio).
fn move_visibly(editor: &mut EditorState, motion: Motion, extend: bool) {
    let forward = motion == Motion::Right;
    let (line, col) = head(editor);
    let before = parse_line(&editor.buffer().line_text(line)).visible_offset(col);
    for _ in 0..16 {
        let previous = editor.selection().head;
        editor.move_cursor(motion, extend);
        let (new_line, new_col) = head(editor);
        if editor.selection().head == previous || new_line != line {
            break;
        }
        let markup = parse_line(&editor.buffer().line_text(new_line));
        let snapped = markup.snap(new_col, forward);
        if markup.visible_offset(snapped) != before {
            if snapped != new_col {
                let start = editor.buffer().line_start(new_line);
                let anchor = if extend {
                    editor.selection().anchor
                } else {
                    start + snapped
                };
                editor.set_selection(Selection::new(anchor, start + snapped));
            }
            break;
        }
    }
}

/// Retroceso: al principio de una viñeta o un título quita la marca (como en el Bloc de
/// notas); detrás de una marca oculta borra el carácter visible anterior.
fn backspace(editor: &mut EditorState) -> Result<(), CoreError> {
    let (line, col) = head(editor);
    let markup = parse_line(&editor.buffer().line_text(line));
    let start = editor.buffer().line_start(line);
    if markup.prefix > 0 && col <= markup.prefix {
        editor.replace_ranges(&[(start..start + markup.prefix, String::new())])?;
        editor.set_cursor(start, false);
        return Ok(());
    }
    if let Some(run) = markup
        .hidden_runs()
        .into_iter()
        .find(|run| run.end == col && run.start > 0)
    {
        editor.set_cursor(start + run.start, false);
    }
    editor.backspace()
}

/// Intro en una lista: continúa la lista o, en un elemento vacío, la termina.
fn newline(editor: &mut EditorState) -> Option<Result<(), CoreError>> {
    let (line, _) = head(editor);
    let text = editor.buffer().line_text(line);
    match continue_list(&text)? {
        ListContinuation::Continue(prefix) => Some(editor.insert_text(&format!("\n{prefix}"))),
        ListContinuation::End(prefix) => {
            let start = editor.buffer().line_start(line);
            Some(
                editor
                    .replace_ranges(&[(start..start + prefix, String::new())])
                    .map(|_| editor.set_cursor(start, false)),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, cursor: usize, action: EditorAction) -> EditorState {
        let mut editor = EditorState::from_text(text);
        editor.set_cursor(cursor, false);
        if apply(&action, &mut editor).is_none() {
            match action {
                EditorAction::Delete => editor.delete_forward().unwrap(),
                _ => unreachable!(),
            }
        }
        editor
    }

    fn right() -> EditorAction {
        EditorAction::Move {
            motion: Motion::Right,
            extend: false,
        }
    }

    #[test]
    fn arrows_skip_hidden_markers() {
        // «a **b**»: tras «a » el cursor salta los asteriscos de una vez.
        let editor = run("a **b**", 2, right());
        assert_eq!(editor.selection().head, 5);
        let editor = run(
            "a **b**",
            5,
            EditorAction::Move {
                motion: Motion::Left,
                extend: false,
            },
        );
        // Delante de la «b», dentro de la negrita (lo que se escriba irá en negrita).
        assert_eq!(editor.selection().head, 4);
    }

    #[test]
    fn backspace_removes_list_markers_and_keeps_bold_markers() {
        let editor = run("- pan", 2, EditorAction::Backspace);
        assert_eq!(editor.text(), "pan");
        let editor = run("## Título", 3, EditorAction::Backspace);
        assert_eq!(editor.text(), "Título");
        // Detrás de «**b**» borra la «b», no un asterisco.
        let editor = run("a **b**", 7, EditorAction::Backspace);
        assert_eq!(editor.text(), "a ****");
    }

    #[test]
    fn delete_skips_opening_markers() {
        let editor = run("a **b**", 2, EditorAction::Delete);
        assert_eq!(editor.text(), "a ****");
    }

    #[test]
    fn enter_continues_and_ends_lists() {
        let editor = run("- pan", 5, EditorAction::Newline);
        assert_eq!(editor.text(), "- pan\n- ");
        let editor = run("1. uno\n2. ", 10, EditorAction::Newline);
        assert_eq!(editor.text(), "1. uno\n");
    }

    #[test]
    fn cursor_never_rests_before_a_bullet() {
        let mut editor = EditorState::from_text("- pan");
        snap_cursor(&mut editor);
        assert_eq!(editor.selection().head, 2);
    }
}
