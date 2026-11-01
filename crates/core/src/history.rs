//! Historial de deshacer/rehacer basado en transacciones.

use crate::{Cursors, Edit};

/// Tipo de edición, usado para decidir si una edición se agrupa con la anterior.
///
/// Solo se agrupan ediciones consecutivas del mismo tipo: así una palabra tecleada
/// seguida, o una serie de retrocesos, se deshace de una vez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditKind {
    /// Caracteres de palabra tecleados uno a uno.
    Word,
    /// Espacios o tabuladores tecleados uno a uno.
    Space,
    /// Borrado hacia atrás de un carácter.
    Backspace,
    /// Borrado hacia delante de un carácter.
    DeleteForward,
}

/// Grupo de ediciones que se deshace y rehace como una unidad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Identificador único, usado para saber si el documento ha cambiado desde que se guardó.
    pub id: u64,
    pub edits: Vec<Edit>,
    /// Selección antes de aplicar la transacción (se restaura al deshacer).
    pub selection_before: Cursors,
    /// Selección después de aplicarla (se restaura al rehacer).
    pub selection_after: Cursors,
}

/// Pilas de deshacer y rehacer.
#[derive(Debug, Clone, Default)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    /// Tipo de la transacción en la cima si todavía admite agruparse.
    open_group: Option<EditKind>,
    next_id: u64,
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra ediciones ya aplicadas al buffer. Vacía la pila de rehacer.
    ///
    /// Si `kind` coincide con el grupo abierto y la edición continúa donde terminó la
    /// anterior, se añade a la misma transacción.
    pub fn record(
        &mut self,
        edits: Vec<Edit>,
        selection_before: Cursors,
        selection_after: Cursors,
        kind: Option<EditKind>,
    ) {
        if edits.is_empty() {
            return;
        }
        self.redo.clear();

        if let (Some(kind), Some(top)) = (kind, self.undo.last_mut())
            && self.open_group == Some(kind)
            && top.selection_after == selection_before
        {
            top.edits.extend(edits);
            top.selection_after = selection_after;
            return;
        }

        self.next_id += 1;
        self.undo.push(Transaction {
            id: self.next_id,
            edits,
            selection_before,
            selection_after,
        });
        self.open_group = kind;
    }

    /// Cierra el grupo abierto: la próxima edición creará una transacción nueva.
    pub fn seal(&mut self) {
        self.open_group = None;
    }

    /// Saca la última transacción para deshacerla. Quien llama aplica las inversas.
    pub fn undo(&mut self) -> Option<&Transaction> {
        self.open_group = None;
        let transaction = self.undo.pop()?;
        self.redo.push(transaction);
        self.redo.last()
    }

    /// Recupera la última transacción deshecha. Quien llama la vuelve a aplicar.
    pub fn redo(&mut self) -> Option<&Transaction> {
        self.open_group = None;
        let transaction = self.redo.pop()?;
        self.undo.push(transaction);
        self.undo.last()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Identifica el estado actual del texto: el id de la última transacción aplicada,
    /// o 0 si no hay ninguna. Dos estados con la misma revisión tienen el mismo texto.
    pub fn revision(&self) -> u64 {
        self.undo.last().map_or(0, |t| t.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Selection;

    fn cur(at: usize) -> Cursors {
        Selection::cursor(at).into()
    }

    fn insert(at: usize, text: &str) -> Vec<Edit> {
        vec![Edit::Insert {
            at,
            text: text.into(),
        }]
    }

    #[test]
    fn same_kind_contiguous_edits_are_grouped() {
        let mut h = History::new();
        h.record(insert(0, "a"), cur(0), cur(1), Some(EditKind::Word));
        h.record(insert(1, "b"), cur(1), cur(2), Some(EditKind::Word));
        let t = h.undo().unwrap();
        assert_eq!(t.edits.len(), 2);
        assert_eq!(t.selection_before, cur(0));
        assert!(!h.can_undo());
    }

    #[test]
    fn different_kinds_are_not_grouped() {
        let mut h = History::new();
        h.record(insert(0, "a"), cur(0), cur(1), Some(EditKind::Word));
        h.record(insert(1, " "), cur(1), cur(2), Some(EditKind::Space));
        h.undo();
        assert!(h.can_undo());
    }

    #[test]
    fn seal_closes_group() {
        let mut h = History::new();
        h.record(insert(0, "a"), cur(0), cur(1), Some(EditKind::Word));
        h.seal();
        h.record(insert(1, "b"), cur(1), cur(2), Some(EditKind::Word));
        h.undo();
        assert!(h.can_undo());
    }

    #[test]
    fn non_contiguous_edits_are_not_grouped() {
        let mut h = History::new();
        h.record(insert(0, "a"), cur(0), cur(1), Some(EditKind::Word));
        h.record(insert(5, "b"), cur(5), cur(6), Some(EditKind::Word));
        h.undo();
        assert!(h.can_undo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut h = History::new();
        h.record(insert(0, "a"), cur(0), cur(1), None);
        h.undo();
        assert!(h.can_redo());
        h.record(insert(0, "b"), cur(0), cur(1), None);
        assert!(!h.can_redo());
    }

    #[test]
    fn revision_tracks_state() {
        let mut h = History::new();
        assert_eq!(h.revision(), 0);
        h.record(insert(0, "a"), cur(0), cur(1), None);
        let saved = h.revision();
        h.record(insert(1, "b"), cur(1), cur(2), None);
        assert_ne!(h.revision(), saved);
        h.undo();
        assert_eq!(h.revision(), saved);
        h.undo();
        assert_eq!(h.revision(), 0);
        h.redo();
        assert_eq!(h.revision(), saved);
    }

    #[test]
    fn empty_edits_are_ignored() {
        let mut h = History::new();
        h.record(Vec::new(), cur(0), cur(0), None);
        assert!(!h.can_undo());
    }
}
