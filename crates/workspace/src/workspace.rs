//! Colección de documentos abiertos, orden de pestañas y pestaña activa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lorca_core::{LoadedText, load_bytes, save_bytes};

use crate::{Document, DocumentId, WorkspaceError};

/// Documentos abiertos y estado de las pestañas.
///
/// Invariantes: cada id de `tabs` existe en `documents` y viceversa; `active`, si existe,
/// está en `tabs`; `active` es `None` solo cuando no hay pestañas.
#[derive(Debug, Default)]
pub struct Workspace {
    documents: HashMap<DocumentId, Document>,
    tabs: Vec<DocumentId>,
    active: Option<DocumentId>,
    next_id: u64,
}

impl Workspace {
    pub fn new() -> Self {
        Self::default()
    }

    // --- Consultas --------------------------------------------------------------------

    /// Ids de los documentos en el orden de las pestañas.
    pub fn tabs(&self) -> &[DocumentId] {
        &self.tabs
    }

    /// Documentos en el orden de las pestañas.
    pub fn documents(&self) -> impl Iterator<Item = &Document> {
        self.tabs.iter().filter_map(|id| self.documents.get(id))
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active_id(&self) -> Option<DocumentId> {
        self.active
    }

    pub fn active(&self) -> Option<&Document> {
        self.active.and_then(|id| self.documents.get(&id))
    }

    pub fn active_mut(&mut self) -> Option<&mut Document> {
        self.active.and_then(|id| self.documents.get_mut(&id))
    }

    pub fn document(&self, id: DocumentId) -> Option<&Document> {
        self.documents.get(&id)
    }

    pub fn document_mut(&mut self, id: DocumentId) -> Option<&mut Document> {
        self.documents.get_mut(&id)
    }

    /// Documento abierto con esa ruta, si lo hay.
    pub fn find_by_path(&self, path: &Path) -> Option<DocumentId> {
        let path = normalize_path(path);
        self.documents()
            .find(|d| d.path() == Some(path.as_path()))
            .map(Document::id)
    }

    /// Documentos con cambios sin guardar, en el orden de las pestañas.
    pub fn modified_documents(&self) -> Vec<DocumentId> {
        self.documents()
            .filter(|d| d.is_modified())
            .map(Document::id)
            .collect()
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.documents().any(Document::is_modified)
    }

    // --- Pestañas ---------------------------------------------------------------------

    /// Crea un documento sin título vacío, lo añade al final y lo activa.
    pub fn new_document(&mut self) -> DocumentId {
        let number = self.lowest_free_untitled_number();
        let id = self.allocate_id();
        self.insert(Document::untitled(id, number))
    }

    /// Abre un texto ya cargado. Si la ruta ya estaba abierta, solo activa su pestaña.
    ///
    /// Si la única pestaña es un documento sin título vacío y sin cambios, se sustituye.
    pub fn open_loaded(&mut self, path: &Path, loaded: LoadedText) -> DocumentId {
        let path = normalize_path(path);
        if let Some(id) = self.find_by_path(&path) {
            self.active = Some(id);
            return id;
        }
        if let [only] = self.tabs[..]
            && self
                .documents
                .get(&only)
                .is_some_and(Document::is_pristine_untitled)
        {
            self.close(only);
        }
        let id = self.allocate_id();
        self.insert(Document::from_loaded(id, path, loaded))
    }

    /// Lee un archivo de disco y lo abre (ver [`Workspace::open_loaded`]).
    pub fn open_file(&mut self, path: &Path) -> Result<DocumentId, WorkspaceError> {
        if let Some(id) = self.find_by_path(path) {
            self.active = Some(id);
            return Ok(id);
        }
        let bytes = std::fs::read(path).map_err(|source| WorkspaceError::Read {
            path: path.to_owned(),
            source,
        })?;
        Ok(self.open_loaded(path, load_bytes(&bytes)))
    }

    /// Cierra un documento sin preguntar y lo devuelve.
    ///
    /// Si era la pestaña activa, se activa la que queda en su lugar (la de la derecha) o,
    /// si era la última, la de la izquierda.
    pub fn close(&mut self, id: DocumentId) -> Option<Document> {
        let index = self.tabs.iter().position(|t| *t == id)?;
        self.tabs.remove(index);
        if self.active == Some(id) {
            self.active = self.tabs.get(index).or_else(|| self.tabs.last()).copied();
        }
        self.documents.remove(&id)
    }

    /// Activa una pestaña. Devuelve `false` si el documento no existe.
    pub fn activate(&mut self, id: DocumentId) -> bool {
        if self.documents.contains_key(&id) {
            self.active = Some(id);
            true
        } else {
            false
        }
    }

    /// Activa la pestaña siguiente (vuelve a la primera tras la última).
    pub fn activate_next(&mut self) {
        self.cycle(1);
    }

    /// Activa la pestaña anterior (vuelve a la última tras la primera).
    pub fn activate_previous(&mut self) {
        self.cycle(-1);
    }

    // --- Guardado ---------------------------------------------------------------------

    /// Guarda el documento en su ruta actual.
    pub fn save(&mut self, id: DocumentId) -> Result<(), WorkspaceError> {
        let path = self
            .document(id)
            .ok_or(WorkspaceError::UnknownDocument(id))?
            .path()
            .ok_or(WorkspaceError::NoPath)?
            .to_owned();
        self.save_as(id, &path)
    }

    /// Guarda el documento en `path` y lo asocia a esa ruta.
    pub fn save_as(&mut self, id: DocumentId, path: &Path) -> Result<(), WorkspaceError> {
        let document = self
            .documents
            .get_mut(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))?;
        let bytes = save_bytes(&document.editor.text(), document.format())?;
        std::fs::write(path, bytes).map_err(|source| WorkspaceError::Write {
            path: path.to_owned(),
            source,
        })?;
        document.mark_saved(normalize_path(path));
        Ok(())
    }

    // --- Auxiliares -------------------------------------------------------------------

    fn allocate_id(&mut self) -> DocumentId {
        self.next_id += 1;
        DocumentId(self.next_id)
    }

    /// Crea un documento con un id nuevo, lo añade al final y lo activa.
    pub(crate) fn push_document(
        &mut self,
        make: impl FnOnce(DocumentId) -> Document,
    ) -> DocumentId {
        let id = self.allocate_id();
        self.insert(make(id))
    }

    fn insert(&mut self, document: Document) -> DocumentId {
        let id = document.id();
        self.documents.insert(id, document);
        self.tabs.push(id);
        self.active = Some(id);
        id
    }

    fn cycle(&mut self, step: isize) {
        let Some(active) = self.active else { return };
        let Some(index) = self.tabs.iter().position(|t| *t == active) else {
            return;
        };
        let len = self.tabs.len();
        let next = if step >= 0 {
            (index + 1) % len
        } else {
            (index + len - 1) % len
        };
        self.active = self.tabs.get(next).copied();
    }

    fn lowest_free_untitled_number(&self) -> u32 {
        let used: Vec<u32> = self
            .documents
            .values()
            .filter_map(Document::untitled_number)
            .collect();
        (1..).find(|n| !used.contains(n)).unwrap_or(1)
    }
}

/// Ruta absoluta y canónica si existe; si no, la ruta tal cual.
fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_core::{LineEnding, TextEncoding};

    fn loaded(text: &str) -> LoadedText {
        load_bytes(text.as_bytes())
    }

    #[test]
    fn new_documents_get_increasing_untitled_names() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        assert_eq!(ws.document(a).unwrap().display_name(), "Sin título 1");
        assert_eq!(ws.document(b).unwrap().display_name(), "Sin título 2");
        assert_eq!(ws.active_id(), Some(b));
        assert_eq!(ws.tabs(), &[a, b]);
    }

    #[test]
    fn untitled_numbers_are_reused() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        ws.new_document();
        ws.close(a);
        let c = ws.new_document();
        assert_eq!(ws.document(c).unwrap().display_name(), "Sin título 1");
    }

    #[test]
    fn open_loaded_sets_name_language_and_format() {
        let mut ws = Workspace::new();
        let id = ws.open_loaded(Path::new("/no/existe/poema.md"), loaded("verso\r\n"));
        let doc = ws.document(id).unwrap();
        assert_eq!(doc.display_name(), "poema.md");
        assert_eq!(doc.language().name(), "Markdown");
        assert_eq!(doc.format().line_ending, LineEnding::CrLf);
        assert_eq!(doc.format().encoding, TextEncoding::Utf8);
        assert_eq!(doc.editor.text(), "verso\n");
        assert!(!doc.is_modified());
    }

    #[test]
    fn opening_same_path_twice_activates_existing() {
        let mut ws = Workspace::new();
        let a = ws.open_loaded(Path::new("/x/a.txt"), loaded("a"));
        ws.new_document();
        let again = ws.open_loaded(Path::new("/x/a.txt"), loaded("otro"));
        assert_eq!(again, a);
        assert_eq!(ws.active_id(), Some(a));
        assert_eq!(ws.len(), 2);
    }

    #[test]
    fn opening_replaces_single_pristine_untitled() {
        let mut ws = Workspace::new();
        ws.new_document();
        let id = ws.open_loaded(Path::new("/x/a.txt"), loaded("a"));
        assert_eq!(ws.tabs(), &[id]);
    }

    #[test]
    fn opening_keeps_modified_untitled() {
        let mut ws = Workspace::new();
        let untitled = ws.new_document();
        ws.active_mut()
            .unwrap()
            .editor
            .insert_text("borrador")
            .unwrap();
        ws.open_loaded(Path::new("/x/a.txt"), loaded("a"));
        assert_eq!(ws.len(), 2);
        assert_eq!(ws.tabs()[0], untitled);
    }

    #[test]
    fn closing_active_activates_right_then_left() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.activate(b);
        ws.close(b);
        assert_eq!(ws.active_id(), Some(c));
        ws.close(c);
        assert_eq!(ws.active_id(), Some(a));
        ws.close(a);
        assert_eq!(ws.active_id(), None);
        assert!(ws.is_empty());
    }

    #[test]
    fn closing_inactive_keeps_active() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        ws.close(a);
        assert_eq!(ws.active_id(), Some(b));
    }

    #[test]
    fn closing_unknown_returns_none() {
        let mut ws = Workspace::new();
        assert!(ws.close(DocumentId(42)).is_none());
        assert!(!ws.activate(DocumentId(42)));
    }

    #[test]
    fn cycling_tabs_wraps_around() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.activate_next();
        assert_eq!(ws.active_id(), Some(a));
        ws.activate_next();
        assert_eq!(ws.active_id(), Some(b));
        ws.activate_previous();
        ws.activate_previous();
        assert_eq!(ws.active_id(), Some(c));
    }

    #[test]
    fn cycling_empty_workspace_does_nothing() {
        let mut ws = Workspace::new();
        ws.activate_next();
        assert_eq!(ws.active_id(), None);
    }

    #[test]
    fn modification_tracking() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        assert!(!ws.has_unsaved_changes());
        ws.active_mut().unwrap().editor.insert_text("x").unwrap();
        assert_eq!(ws.modified_documents(), vec![a]);
        ws.active_mut().unwrap().editor.undo().unwrap();
        assert!(!ws.has_unsaved_changes());
        ws.active_mut().unwrap().mark_modified();
        assert!(ws.has_unsaved_changes());
    }

    #[test]
    fn save_without_path_fails() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        assert!(matches!(ws.save(a), Err(WorkspaceError::NoPath)));
        assert!(matches!(
            ws.save(DocumentId(99)),
            Err(WorkspaceError::UnknownDocument(_))
        ));
    }

    #[test]
    fn save_as_writes_file_and_clears_modified() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("romance.rs");
        let mut ws = Workspace::new();
        let id = ws.new_document();
        ws.active_mut()
            .unwrap()
            .editor
            .insert_text("fn main() {}\n")
            .unwrap();
        ws.save_as(id, &path).unwrap();

        let doc = ws.document(id).unwrap();
        assert!(!doc.is_modified());
        assert_eq!(doc.display_name(), "romance.rs");
        assert_eq!(doc.language().name(), "Rust");
        assert_eq!(doc.untitled_number(), None);
        let expected = LineEnding::platform_default()
            .apply("fn main() {}\n")
            .into_owned();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    }

    #[test]
    fn open_and_save_preserves_crlf_and_encoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("viejo.txt");
        std::fs::write(&path, b"canci\xF3n\r\nverde\r\n").unwrap();

        let mut ws = Workspace::new();
        let id = ws.open_file(&path).unwrap();
        assert_eq!(
            ws.document(id).unwrap().format().encoding,
            TextEncoding::Windows1252
        );
        let doc = ws.document_mut(id).unwrap();
        doc.editor
            .move_cursor(lorca_core::Motion::DocumentEnd, false);
        doc.editor.insert_text("luna\n").unwrap();
        assert!(doc.is_modified());
        ws.save(id).unwrap();

        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"canci\xF3n\r\nverde\r\nluna\r\n"
        );
        assert!(!ws.has_unsaved_changes());
        // Reabrir la misma ruta (aunque sea por otro camino) activa la pestaña existente.
        assert_eq!(
            ws.open_file(&dir.path().join(".").join("viejo.txt"))
                .unwrap(),
            id
        );
    }

    #[test]
    fn open_missing_file_fails() {
        let mut ws = Workspace::new();
        let err = ws.open_file(Path::new("/no/existe/nada.txt")).unwrap_err();
        assert!(matches!(err, WorkspaceError::Read { .. }));
        assert!(ws.is_empty());
    }
}
