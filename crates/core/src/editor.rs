//! Estado de edición de un documento: buffer, selección e historial.

use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    Bookmarks, Buffer, CoreError, Edit, EditKind, History, LineEnding, Position, Selection,
};

/// Movimientos del cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordLeft,
    WordRight,
    /// Inicio "inteligente": primero el primer carácter no blanco, luego la columna 0.
    LineStart,
    LineEnd,
    DocumentStart,
    DocumentEnd,
    /// Sube el número de líneas indicado.
    PageUp(usize),
    /// Baja el número de líneas indicado.
    PageDown(usize),
}

/// Buffer, selección e historial de un documento, con las operaciones de edición.
///
/// Cada operación que modifica el texto se registra en el historial como una
/// transacción, de modo que deshacer/rehacer restaura texto y selección.
#[derive(Debug, Clone)]
pub struct EditorState {
    buffer: Buffer,
    selection: Selection,
    history: History,
    /// Columna que se intenta mantener en movimientos verticales consecutivos.
    preferred_column: Option<usize>,
    bookmarks: Bookmarks,
    /// Versión del contenido; ver [`EditorState::version`].
    version: u64,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            buffer: Buffer::default(),
            selection: Selection::default(),
            history: History::default(),
            preferred_column: None,
            bookmarks: Bookmarks::default(),
            version: next_version(),
        }
    }
}

/// Contador global de versiones: así dos estados distintos nunca comparten versión.
static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

fn next_version() -> u64 {
    NEXT_VERSION.fetch_add(1, Ordering::Relaxed)
}

impl EditorState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crea un estado con el texto dado y el cursor al principio.
    pub fn from_text(text: &str) -> Self {
        Self {
            buffer: Buffer::from_text(text),
            ..Self::default()
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn text(&self) -> String {
        self.buffer.to_string()
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    /// Línea y columna de la cabeza de la selección.
    pub fn cursor(&self) -> Position {
        self.buffer.char_to_position(self.selection.head)
    }

    /// Identificador del estado del texto según el historial; ver [`History::revision`].
    ///
    /// Sirve para saber si el texto coincide con lo guardado (deshacer hasta ahí lo
    /// recupera), pero **no** cambia con cada pulsación: las teclas agrupadas en una
    /// transacción comparten revisión. Para cachés, usa [`EditorState::version`].
    pub fn revision(&self) -> u64 {
        self.history.revision()
    }

    /// Versión del contenido: cambia con cada modificación del texto (incluidas las que
    /// se agrupan en la misma transacción, deshacer y rehacer) y es única entre todos los
    /// estados. Clave adecuada para cachés de resaltado, búsqueda, vistas previas…
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Texto seleccionado, o `None` si la selección está vacía.
    pub fn selected_text(&self) -> Option<String> {
        if self.selection.is_empty() {
            return None;
        }
        self.buffer.slice(self.selection.range()).ok()
    }

    // --- Selección y movimiento -------------------------------------------------------

    /// Sustituye la selección (ajustada al buffer) y cierra el grupo de deshacer.
    pub fn set_selection(&mut self, selection: Selection) {
        self.selection = selection.clamped(self.buffer.len_chars());
        self.preferred_column = None;
        self.history.seal();
    }

    /// Coloca el cursor en `index`. Con `extend`, mantiene el ancla y extiende la selección.
    pub fn set_cursor(&mut self, index: usize, extend: bool) {
        let anchor = if extend { self.selection.anchor } else { index };
        self.set_selection(Selection::new(anchor, index));
    }

    pub fn select_all(&mut self) {
        self.set_selection(Selection::new(0, self.buffer.len_chars()));
    }

    /// Selecciona la línea completa, incluido su salto final.
    pub fn select_line(&mut self, line: usize) {
        let start = self.buffer.line_start(line);
        let end = self.buffer.line_start(line + 1);
        let end = if end > start {
            end
        } else {
            self.buffer.len_chars()
        };
        self.set_selection(Selection::new(start, end));
    }

    /// Selecciona la palabra (o el bloque de blancos o de signos) que contiene `index`.
    pub fn select_word_at(&mut self, index: usize) {
        let len = self.buffer.len_chars();
        let index = index.min(len);
        // Al final de una palabra se toma el carácter anterior.
        let probe = match self.buffer.char_at(index) {
            Some(c) if c != '\n' => index,
            _ if index > 0 => index - 1,
            _ => {
                self.set_cursor(index, false);
                return;
            }
        };
        let Some(class) = self.buffer.char_at(probe).map(CharClass::of) else {
            self.set_cursor(index, false);
            return;
        };
        if class == CharClass::Newline {
            self.set_cursor(index, false);
            return;
        }
        let mut start = probe;
        while start > 0 && self.class_at(start - 1) == Some(class) {
            start -= 1;
        }
        let mut end = probe;
        while end < len && self.class_at(end) == Some(class) {
            end += 1;
        }
        self.set_selection(Selection::new(start, end));
    }

    /// Mueve el cursor. Con `extend`, extiende la selección en lugar de colapsarla.
    pub fn move_cursor(&mut self, motion: Motion, extend: bool) {
        let selection = self.selection;
        let head = selection.head;
        let len = self.buffer.len_chars();
        let collapse = !extend && !selection.is_empty();

        let current_column = self.buffer.char_to_position(head).column;
        let column = self.preferred_column.unwrap_or(current_column);

        let (target, vertical) = match motion {
            Motion::Left if collapse => (selection.start(), false),
            Motion::Right if collapse => (selection.end(), false),
            Motion::Left => (head.saturating_sub(1), false),
            Motion::Right => ((head + 1).min(len), false),
            Motion::Up => (self.vertical_target(head, column, -1), true),
            Motion::Down => (self.vertical_target(head, column, 1), true),
            Motion::PageUp(lines) => (self.vertical_target(head, column, -signed(lines)), true),
            Motion::PageDown(lines) => (self.vertical_target(head, column, signed(lines)), true),
            Motion::WordLeft => (self.word_left(head), false),
            Motion::WordRight => (self.word_right(head), false),
            Motion::LineStart => (self.smart_line_start(head), false),
            Motion::LineEnd => (self.buffer.line_end(self.buffer.char_to_line(head)), false),
            Motion::DocumentStart => (0, false),
            Motion::DocumentEnd => (len, false),
        };

        let anchor = if extend { selection.anchor } else { target };
        self.set_selection(Selection::new(anchor, target));
        if vertical {
            self.preferred_column = Some(column);
        }
    }

    // --- Edición ----------------------------------------------------------------------

    /// Inserta texto sustituyendo la selección. Los finales de línea se normalizan a LF.
    pub fn insert_text(&mut self, text: &str) -> Result<(), CoreError> {
        let text = LineEnding::normalize(text);
        let selection = self.selection;
        if text.is_empty() && selection.is_empty() {
            return Ok(());
        }

        let kind = if selection.is_empty() {
            kind_of_typed(&text)
        } else {
            None
        };
        let start = selection.start();
        let mut edits = Vec::with_capacity(2);
        if !selection.is_empty() {
            edits.push(Edit::Delete {
                at: start,
                text: self.buffer.slice(selection.range())?,
            });
        }
        let inserted_len = text.chars().count();
        if !text.is_empty() {
            edits.push(Edit::Insert {
                at: start,
                text: text.into_owned(),
            });
        }
        self.apply(edits, Selection::cursor(start + inserted_len), kind)
    }

    /// Inserta un salto de línea manteniendo la indentación de la línea actual.
    pub fn insert_newline(&mut self) -> Result<(), CoreError> {
        let start = self.selection.start();
        let line = self.buffer.char_to_line(start);
        let column = start - self.buffer.line_start(line);
        let indent: String = self
            .buffer
            .line_text(line)
            .chars()
            .take(column)
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        self.insert_text(&format!("\n{indent}"))
    }

    /// Borra la selección o, si está vacía, el carácter anterior al cursor.
    pub fn backspace(&mut self) -> Result<(), CoreError> {
        if !self.selection.is_empty() {
            return self.delete_selection();
        }
        let head = self.selection.head;
        if head == 0 {
            return Ok(());
        }
        let text = self.buffer.slice(head - 1..head)?;
        let edit = Edit::Delete { at: head - 1, text };
        self.apply(
            vec![edit],
            Selection::cursor(head - 1),
            Some(EditKind::Backspace),
        )
    }

    /// Borra la selección o, si está vacía, el carácter siguiente al cursor.
    pub fn delete_forward(&mut self) -> Result<(), CoreError> {
        if !self.selection.is_empty() {
            return self.delete_selection();
        }
        let head = self.selection.head;
        if head >= self.buffer.len_chars() {
            return Ok(());
        }
        let text = self.buffer.slice(head..head + 1)?;
        let edit = Edit::Delete { at: head, text };
        self.apply(
            vec![edit],
            Selection::cursor(head),
            Some(EditKind::DeleteForward),
        )
    }

    /// Borra desde el cursor hasta el inicio de la palabra anterior (o la selección).
    pub fn delete_word_backward(&mut self) -> Result<(), CoreError> {
        if !self.selection.is_empty() {
            return self.delete_selection();
        }
        let head = self.selection.head;
        self.delete_range(self.word_left(head), head)
    }

    /// Borra desde el cursor hasta el final de la palabra siguiente (o la selección).
    pub fn delete_word_forward(&mut self) -> Result<(), CoreError> {
        if !self.selection.is_empty() {
            return self.delete_selection();
        }
        let head = self.selection.head;
        self.delete_range(head, self.word_right(head))
    }

    /// Borra el texto seleccionado (por ejemplo, al cortar).
    pub fn delete_selection(&mut self) -> Result<(), CoreError> {
        let range = self.selection.range();
        self.delete_range(range.start, range.end)
    }

    /// Deshace la última transacción. Devuelve `false` si no había nada que deshacer.
    pub fn undo(&mut self) -> Result<bool, CoreError> {
        self.preferred_column = None;
        let Some(transaction) = self.history.undo() else {
            return Ok(false);
        };
        for edit in transaction.edits.iter().rev() {
            apply_edit(&mut self.buffer, &mut self.bookmarks, &edit.inverse())?;
        }
        self.selection = transaction
            .selection_before
            .clamped(self.buffer.len_chars());
        self.version = next_version();
        Ok(true)
    }

    /// Rehace la última transacción deshecha. Devuelve `false` si no había ninguna.
    pub fn redo(&mut self) -> Result<bool, CoreError> {
        self.preferred_column = None;
        let Some(transaction) = self.history.redo() else {
            return Ok(false);
        };
        for edit in &transaction.edits {
            apply_edit(&mut self.buffer, &mut self.bookmarks, edit)?;
        }
        self.selection = transaction.selection_after.clamped(self.buffer.len_chars());
        self.version = next_version();
        Ok(true)
    }

    /// Sustituye varios rangos en una sola transacción (p. ej. «Reemplazar todo»).
    ///
    /// Los rangos deben estar ordenados y no solaparse. El cursor se mantiene sobre el
    /// mismo texto. Devuelve el número de sustituciones.
    pub fn replace_ranges(
        &mut self,
        replacements: &[(Range<usize>, String)],
    ) -> Result<usize, CoreError> {
        let len = self.buffer.len_chars();
        let mut previous_end = 0;
        for (range, _) in replacements {
            if range.start < previous_end || range.start > range.end || range.end > len {
                return Err(CoreError::InvalidRange {
                    start: range.start,
                    end: range.end,
                    len,
                });
            }
            previous_end = range.end;
        }

        let head = self.selection.head;
        let mut shift: isize = 0;
        let mut edits = Vec::with_capacity(replacements.len() * 2);
        // De atrás hacia delante, para que los índices de los rangos sigan siendo válidos.
        for (range, text) in replacements.iter().rev() {
            let text = LineEnding::normalize(text);
            if !range.is_empty() {
                edits.push(Edit::Delete {
                    at: range.start,
                    text: self.buffer.slice(range.clone())?,
                });
            }
            if !text.is_empty() {
                edits.push(Edit::Insert {
                    at: range.start,
                    text: text.clone().into_owned(),
                });
            }
            if range.end <= head {
                let added = isize::try_from(text.chars().count()).unwrap_or(isize::MAX);
                let removed = isize::try_from(range.len()).unwrap_or(isize::MAX);
                shift = shift.saturating_add(added - removed);
            }
        }
        let new_head = head.saturating_add_signed(shift);
        self.history.seal();
        self.apply(edits, Selection::cursor(new_head), None)?;
        self.history.seal();
        self.selection = self.selection.clamped(self.buffer.len_chars());
        Ok(replacements.len())
    }

    // --- Marcadores --------------------------------------------------------------------

    pub fn bookmarks(&self) -> &Bookmarks {
        &self.bookmarks
    }

    /// Marca o desmarca la línea del cursor. Devuelve `true` si queda marcada.
    pub fn toggle_bookmark(&mut self) -> bool {
        let line = self.cursor().line;
        self.bookmarks.toggle(line)
    }

    /// Marca las líneas que contienen los índices de carácter dados.
    pub fn bookmark_lines_at(&mut self, indices: impl IntoIterator<Item = usize>) {
        for index in indices {
            let line = self.buffer.char_to_line(index);
            self.bookmarks.insert(line);
        }
    }

    pub fn clear_bookmarks(&mut self) {
        self.bookmarks.clear();
    }

    /// Mueve el cursor al siguiente (o anterior) marcador, volviendo al principio.
    /// Devuelve `false` si no hay marcadores.
    pub fn go_to_bookmark(&mut self, forward: bool) -> bool {
        let line = self.cursor().line;
        let target = if forward {
            self.bookmarks.next_after(line, true)
        } else {
            self.bookmarks.previous_before(line, true)
        };
        match target {
            Some(target) => {
                self.set_cursor(self.buffer.line_start(target), false);
                true
            }
            None => false,
        }
    }

    /// Texto de las líneas marcadas, cada una terminada en salto de línea.
    pub fn bookmarked_lines_text(&self) -> String {
        self.bookmarks
            .iter()
            .filter(|&line| line < self.buffer.len_lines())
            .map(|line| format!("{}\n", self.buffer.line_text(line)))
            .collect()
    }

    /// Borra las líneas marcadas en una sola transacción y quita los marcadores.
    pub fn delete_bookmarked_lines(&mut self) -> Result<usize, CoreError> {
        let last = self.buffer.len_lines().saturating_sub(1);
        let mut ranges: Vec<(Range<usize>, String)> = Vec::new();
        for line in self.bookmarks.iter().filter(|&l| l <= last) {
            let start = self.buffer.line_start(line);
            let end = if line < last {
                self.buffer.line_start(line + 1)
            } else {
                self.buffer.len_chars()
            };
            match ranges.last_mut() {
                Some((previous, _)) if previous.end >= start => previous.end = end,
                _ => ranges.push((start..end, String::new())),
            }
        }
        // Si el borrado llega al final del documento, se lleva también el salto de línea
        // anterior, para no dejar una línea vacía al final.
        let len = self.buffer.len_chars();
        if let Some((range, _)) = ranges.last_mut()
            && range.end == len
            && range.start > 0
        {
            range.start -= 1;
        }
        if let [.., (previous, _), (last_range, _)] = ranges.as_mut_slice()
            && previous.end >= last_range.start
        {
            last_range.start = previous.start;
            ranges.remove(ranges.len() - 2);
        }
        let count = self.bookmarks.len();
        self.bookmarks.clear();
        self.replace_ranges(&ranges)?;
        Ok(count)
    }

    // --- Auxiliares -------------------------------------------------------------------

    fn delete_range(&mut self, start: usize, end: usize) -> Result<(), CoreError> {
        if start >= end {
            return Ok(());
        }
        let text = self.buffer.slice(start..end)?;
        self.apply(
            vec![Edit::Delete { at: start, text }],
            Selection::cursor(start),
            None,
        )
    }

    /// Aplica las ediciones y las registra en el historial. Si alguna falla, revierte las
    /// ya aplicadas para no dejar el buffer a medias.
    fn apply(
        &mut self,
        edits: Vec<Edit>,
        after: Selection,
        kind: Option<EditKind>,
    ) -> Result<(), CoreError> {
        for (applied, edit) in edits.iter().enumerate() {
            if let Err(error) = apply_edit(&mut self.buffer, &mut self.bookmarks, edit) {
                for done in edits.iter().take(applied).rev() {
                    // Revertir lo que acabamos de aplicar no puede fallar; si fallara, el
                    // error original es el más útil para quien llama.
                    let _ = apply_edit(&mut self.buffer, &mut self.bookmarks, &done.inverse());
                }
                return Err(error);
            }
        }
        let before = self.selection;
        self.selection = after;
        self.preferred_column = None;
        self.history.record(edits, before, after, kind);
        self.version = next_version();
        Ok(())
    }

    fn class_at(&self, index: usize) -> Option<CharClass> {
        self.buffer.char_at(index).map(CharClass::of)
    }

    fn vertical_target(&self, head: usize, column: usize, delta: isize) -> usize {
        let line = self.buffer.char_to_line(head);
        let last = self.buffer.len_lines().saturating_sub(1);
        match line.checked_add_signed(delta) {
            None => 0,
            Some(target) if target > last => self.buffer.len_chars(),
            Some(target) => self.buffer.position_to_char(Position::new(target, column)),
        }
    }

    fn word_left(&self, from: usize) -> usize {
        let mut i = from;
        while i > 0 && self.class_at(i - 1).is_some_and(CharClass::is_blank) {
            i -= 1;
        }
        if let Some(class) = i.checked_sub(1).and_then(|p| self.class_at(p)) {
            while i > 0 && self.class_at(i - 1) == Some(class) {
                i -= 1;
            }
        }
        i
    }

    fn word_right(&self, from: usize) -> usize {
        let len = self.buffer.len_chars();
        let mut i = from;
        while i < len && self.class_at(i).is_some_and(CharClass::is_blank) {
            i += 1;
        }
        if let Some(class) = self.class_at(i) {
            while i < len && self.class_at(i) == Some(class) {
                i += 1;
            }
        }
        i
    }

    fn smart_line_start(&self, head: usize) -> usize {
        let line = self.buffer.char_to_line(head);
        let start = self.buffer.line_start(line);
        let indent = self
            .buffer
            .line_text(line)
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let first_non_blank = start + indent;
        if head == first_non_blank {
            start
        } else {
            first_non_blank
        }
    }
}

/// Aplica una edición al buffer ajustando antes los marcadores de línea.
fn apply_edit(
    buffer: &mut Buffer,
    bookmarks: &mut Bookmarks,
    edit: &Edit,
) -> Result<(), CoreError> {
    bookmarks.adjust(buffer, edit);
    edit.apply(buffer)
}

/// Clasificación de caracteres para movimientos por palabra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Whitespace,
    Newline,
    Punctuation,
}

impl CharClass {
    fn of(c: char) -> Self {
        if c == '\n' {
            Self::Newline
        } else if c.is_whitespace() {
            Self::Whitespace
        } else if is_word_char(c) {
            Self::Word
        } else {
            Self::Punctuation
        }
    }

    fn is_blank(self) -> bool {
        matches!(self, Self::Whitespace | Self::Newline)
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Tipo de edición para un texto tecleado: solo un carácter suelto se puede agrupar.
fn kind_of_typed(text: &str) -> Option<EditKind> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if is_word_char(c) => Some(EditKind::Word),
        (Some(' ' | '\t'), None) => Some(EditKind::Space),
        _ => None,
    }
}

fn signed(lines: usize) -> isize {
    isize::try_from(lines).unwrap_or(isize::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn type_str(state: &mut EditorState, text: &str) {
        for c in text.chars() {
            if c == '\n' {
                state.insert_newline().unwrap();
            } else {
                state.insert_text(&c.to_string()).unwrap();
            }
        }
    }

    fn at(text: &str, index: usize) -> EditorState {
        let mut s = EditorState::from_text(text);
        s.set_cursor(index, false);
        s
    }

    #[test]
    fn typing_inserts_text_and_moves_cursor() {
        let mut s = EditorState::new();
        type_str(&mut s, "hola");
        assert_eq!(s.text(), "hola");
        assert_eq!(s.selection(), Selection::cursor(4));
        assert_eq!(s.cursor(), Position::new(0, 4));
    }

    #[test]
    fn typed_word_is_one_undo_step() {
        let mut s = EditorState::new();
        type_str(&mut s, "verde que");
        assert!(s.undo().unwrap());
        assert_eq!(s.text(), "verde ");
        assert!(s.undo().unwrap());
        assert_eq!(s.text(), "verde");
        assert!(s.undo().unwrap());
        assert_eq!(s.text(), "");
        assert!(!s.undo().unwrap());
    }

    #[test]
    fn cursor_movement_breaks_undo_group() {
        let mut s = EditorState::new();
        type_str(&mut s, "ab");
        s.move_cursor(Motion::Left, false);
        s.move_cursor(Motion::Right, false);
        type_str(&mut s, "cd");
        s.undo().unwrap();
        assert_eq!(s.text(), "ab");
    }

    #[test]
    fn redo_restores_text_and_selection() {
        let mut s = EditorState::new();
        type_str(&mut s, "luna");
        s.undo().unwrap();
        assert_eq!(s.selection(), Selection::cursor(0));
        assert!(s.redo().unwrap());
        assert_eq!(s.text(), "luna");
        assert_eq!(s.selection(), Selection::cursor(4));
        assert!(!s.redo().unwrap());
    }

    #[test]
    fn insert_replaces_selection_in_one_step() {
        let mut s = EditorState::from_text("verde viento");
        s.set_selection(Selection::new(6, 12));
        s.insert_text("rama").unwrap();
        assert_eq!(s.text(), "verde rama");
        s.undo().unwrap();
        assert_eq!(s.text(), "verde viento");
        assert_eq!(s.selection(), Selection::new(6, 12));
    }

    #[test]
    fn pasted_crlf_is_normalized() {
        let mut s = EditorState::new();
        s.insert_text("a\r\nb").unwrap();
        assert_eq!(s.text(), "a\nb");
        assert_eq!(s.cursor(), Position::new(1, 1));
    }

    #[test]
    fn newline_keeps_indentation() {
        let mut s = EditorState::new();
        type_str(&mut s, "    si\nno");
        assert_eq!(s.text(), "    si\n    no");
    }

    #[test]
    fn backspaces_group_and_undo_together() {
        let mut s = EditorState::from_text("abcd");
        s.set_cursor(4, false);
        s.backspace().unwrap();
        s.backspace().unwrap();
        assert_eq!(s.text(), "ab");
        s.undo().unwrap();
        assert_eq!(s.text(), "abcd");
        assert_eq!(s.selection(), Selection::cursor(4));
    }

    #[test]
    fn backspace_at_start_does_nothing() {
        let mut s = EditorState::from_text("x");
        s.backspace().unwrap();
        assert_eq!(s.text(), "x");
        assert!(!s.can_undo());
    }

    #[test]
    fn delete_forward_and_at_end() {
        let mut s = at("abc", 1);
        s.delete_forward().unwrap();
        s.delete_forward().unwrap();
        assert_eq!(s.text(), "a");
        s.delete_forward().unwrap();
        assert_eq!(s.text(), "a");
        s.undo().unwrap();
        assert_eq!(s.text(), "abc");
    }

    #[test]
    fn backspace_with_selection_deletes_selection() {
        let mut s = EditorState::from_text("granada");
        s.set_selection(Selection::new(0, 4));
        s.backspace().unwrap();
        assert_eq!(s.text(), "ada");
        assert_eq!(s.selection(), Selection::cursor(0));
    }

    #[test]
    fn delete_words() {
        let mut s = at("la casa blanca", 14);
        s.delete_word_backward().unwrap();
        assert_eq!(s.text(), "la casa ");
        let mut s = at("la casa blanca", 2);
        s.delete_word_forward().unwrap();
        assert_eq!(s.text(), "la blanca");
    }

    #[test]
    fn left_right_collapse_selection() {
        let mut s = EditorState::from_text("abcdef");
        s.set_selection(Selection::new(1, 4));
        s.move_cursor(Motion::Left, false);
        assert_eq!(s.selection(), Selection::cursor(1));
        s.set_selection(Selection::new(4, 1));
        s.move_cursor(Motion::Right, false);
        assert_eq!(s.selection(), Selection::cursor(4));
    }

    #[test]
    fn left_right_respect_bounds() {
        let mut s = at("ab", 0);
        s.move_cursor(Motion::Left, false);
        assert_eq!(s.selection().head, 0);
        s.move_cursor(Motion::DocumentEnd, false);
        s.move_cursor(Motion::Right, false);
        assert_eq!(s.selection().head, 2);
    }

    #[test]
    fn extend_keeps_anchor() {
        let mut s = at("abcdef", 2);
        s.move_cursor(Motion::Right, true);
        s.move_cursor(Motion::Right, true);
        assert_eq!(s.selection(), Selection::new(2, 4));
        assert_eq!(s.selected_text().as_deref(), Some("cd"));
    }

    #[test]
    fn vertical_motion_keeps_preferred_column() {
        let mut s = at("abcdef\nab\nabcdef", 5);
        s.move_cursor(Motion::Down, false);
        assert_eq!(s.cursor(), Position::new(1, 2));
        s.move_cursor(Motion::Down, false);
        assert_eq!(s.cursor(), Position::new(2, 5));
        s.move_cursor(Motion::Up, false);
        s.move_cursor(Motion::Up, false);
        assert_eq!(s.cursor(), Position::new(0, 5));
    }

    #[test]
    fn vertical_motion_at_edges_goes_to_document_bounds() {
        let mut s = at("abc\ndef", 1);
        s.move_cursor(Motion::Up, false);
        assert_eq!(s.selection().head, 0);
        s.set_cursor(5, false);
        s.move_cursor(Motion::Down, false);
        assert_eq!(s.selection().head, 7);
    }

    #[test]
    fn page_motion() {
        let text = (0..20)
            .map(|i| format!("línea {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut s = at(&text, 0);
        s.move_cursor(Motion::PageDown(10), false);
        assert_eq!(s.cursor().line, 10);
        s.move_cursor(Motion::PageUp(3), false);
        assert_eq!(s.cursor().line, 7);
    }

    #[test]
    fn word_motion() {
        let mut s = at("fn main() {", 0);
        s.move_cursor(Motion::WordRight, false);
        assert_eq!(s.selection().head, 2);
        s.move_cursor(Motion::WordRight, false);
        assert_eq!(s.selection().head, 7);
        s.move_cursor(Motion::WordRight, false);
        assert_eq!(s.selection().head, 9);
        s.move_cursor(Motion::WordLeft, false);
        assert_eq!(s.selection().head, 7);
        s.move_cursor(Motion::WordLeft, false);
        assert_eq!(s.selection().head, 3);
    }

    #[test]
    fn smart_home_toggles() {
        let mut s = at("    código", 8);
        s.move_cursor(Motion::LineStart, false);
        assert_eq!(s.selection().head, 4);
        s.move_cursor(Motion::LineStart, false);
        assert_eq!(s.selection().head, 0);
        s.move_cursor(Motion::LineEnd, false);
        assert_eq!(s.selection().head, 10);
    }

    #[test]
    fn select_word_and_all() {
        let mut s = EditorState::from_text("bodas de_sangre!");
        s.select_word_at(10);
        assert_eq!(s.selected_text().as_deref(), Some("de_sangre"));
        s.select_word_at(15);
        assert_eq!(s.selected_text().as_deref(), Some("!"));
        s.select_word_at(16);
        assert_eq!(s.selected_text().as_deref(), Some("!"));
        s.select_all();
        assert_eq!(s.selection(), Selection::new(0, 16));
    }

    #[test]
    fn select_line_includes_newline() {
        let mut s = EditorState::from_text("uno\ndos\ntres");
        s.select_line(1);
        assert_eq!(s.selected_text().as_deref(), Some("dos\n"));
        s.select_line(2);
        assert_eq!(s.selected_text().as_deref(), Some("tres"));
    }

    #[test]
    fn select_word_on_empty_line_places_cursor() {
        let mut s = EditorState::from_text("a\n\nb");
        s.select_word_at(2);
        assert_eq!(s.selection(), Selection::cursor(2));
    }

    #[test]
    fn revision_returns_to_saved_after_undo() {
        let mut s = EditorState::new();
        type_str(&mut s, "a");
        let saved = s.revision();
        s.move_cursor(Motion::Left, false);
        type_str(&mut s, "b");
        assert_ne!(s.revision(), saved);
        s.undo().unwrap();
        assert_eq!(s.revision(), saved);
    }

    #[test]
    fn version_changes_on_every_edit_even_when_grouped() {
        let mut s = EditorState::new();
        let initial = s.version();
        s.insert_text("a").unwrap();
        let after_a = s.version();
        s.insert_text("b").unwrap();
        // Misma transacción (misma revisión) pero distinta versión.
        assert_ne!(s.version(), after_a);
        assert_ne!(after_a, initial);
        let before_undo = s.version();
        s.undo().unwrap();
        assert_ne!(s.version(), before_undo);
        // Mover el cursor no cambia el contenido.
        let version = s.version();
        s.move_cursor(Motion::Right, false);
        assert_eq!(s.version(), version);
    }

    #[test]
    fn versions_are_unique_between_states() {
        assert_ne!(
            EditorState::from_text("a").version(),
            EditorState::from_text("a").version()
        );
    }

    #[test]
    fn set_selection_is_clamped() {
        let mut s = EditorState::from_text("abc");
        s.set_selection(Selection::new(1, 99));
        assert_eq!(s.selection(), Selection::new(1, 3));
    }

    #[test]
    fn replace_ranges_is_one_undo_step_and_keeps_cursor() {
        let mut s = at("uno dos uno tres uno", 17);
        let n = s
            .replace_ranges(&[
                (0..3, "1".into()),
                (8..11, "1".into()),
                (17..20, "UNO".into()),
            ])
            .unwrap();
        assert_eq!(n, 3);
        assert_eq!(s.text(), "1 dos 1 tres UNO");
        // El cursor estaba al principio del último «uno»: sigue ahí.
        assert_eq!(s.selection(), Selection::cursor(13));
        s.undo().unwrap();
        assert_eq!(s.text(), "uno dos uno tres uno");
        assert!(!s.can_undo());
    }

    #[test]
    fn replace_ranges_rejects_overlaps() {
        let mut s = EditorState::from_text("abcdef");
        let result = s.replace_ranges(&[(0..3, String::new()), (2..4, String::new())]);
        assert!(result.is_err());
        assert_eq!(s.text(), "abcdef");
    }

    #[test]
    fn bookmarks_follow_edits_and_undo() {
        let mut s = at("a\nb\nc", 2);
        assert!(s.toggle_bookmark());
        s.set_cursor(0, false);
        s.insert_newline().unwrap();
        assert!(s.bookmarks().contains(2));
        s.undo().unwrap();
        assert!(s.bookmarks().contains(1));
    }

    #[test]
    fn bookmark_navigation_wraps() {
        let mut s = EditorState::from_text("0\n1\n2\n3");
        s.bookmark_lines_at([2, 6]);
        s.set_cursor(0, false);
        assert!(s.go_to_bookmark(true));
        assert_eq!(s.cursor().line, 1);
        assert!(s.go_to_bookmark(true));
        assert_eq!(s.cursor().line, 3);
        assert!(s.go_to_bookmark(true));
        assert_eq!(s.cursor().line, 1);
        assert!(s.go_to_bookmark(false));
        assert_eq!(s.cursor().line, 3);
    }

    #[test]
    fn copy_and_delete_bookmarked_lines() {
        let mut s = EditorState::from_text("uno\ndos\ntres\ncuatro");
        s.bookmark_lines_at([0, 9, 14]);
        assert_eq!(s.bookmarked_lines_text(), "uno\ntres\ncuatro\n");
        assert_eq!(s.delete_bookmarked_lines().unwrap(), 3);
        assert_eq!(s.text(), "dos");
        assert!(s.bookmarks().is_empty());
        s.undo().unwrap();
        assert_eq!(s.text(), "uno\ndos\ntres\ncuatro");
    }

    #[test]
    fn multibyte_text_editing() {
        let mut s = EditorState::from_text("añ🌙z");
        s.set_cursor(3, false);
        s.backspace().unwrap();
        assert_eq!(s.text(), "añz");
        s.undo().unwrap();
        assert_eq!(s.text(), "añ🌙z");
    }
}
