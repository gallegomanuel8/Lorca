//! Colección de documentos abiertos, orden de pestañas y pestaña activa.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lorca_core::{
    LineEnding, LoadedText, TextEncoding, TextFormat, decode_as, load_bytes, save_bytes,
};

use crate::{Document, DocumentId, WorkspaceError};

/// Documentos abiertos y estado de las pestañas.
///
/// Máximo de documentos en pantalla dividida.
pub const MAX_PANES: usize = 4;

/// Invariantes: cada id de `tabs` existe en `documents` y viceversa; `active`, si existe,
/// está en `tabs`; `active` es `None` solo cuando no hay pestañas.
#[derive(Debug, Default)]
pub struct Workspace {
    documents: HashMap<DocumentId, Document>,
    tabs: Vec<DocumentId>,
    active: Option<DocumentId>,
    /// Pestañas seleccionadas con Cmd/Ctrl+clic, en el orden en que se eligieron.
    selected: Vec<DocumentId>,
    /// Documentos mostrados en pantalla dividida, de izquierda a derecha (vacío = sin
    /// dividir; si no, entre 2 y [`MAX_PANES`]). El activo siempre es uno de ellos.
    panes: Vec<DocumentId>,
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
            self.set_active(id);
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
            self.set_active(id);
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
        self.selected.retain(|s| *s != id);
        if let Some(pane) = self.panes.iter().position(|p| *p == id) {
            self.panes.remove(pane);
            if self.active == Some(id) {
                self.active = self
                    .panes
                    .get(pane.min(self.panes.len().saturating_sub(1)))
                    .copied();
            }
            if self.panes.len() < 2 {
                self.panes.clear();
            }
        }
        if self.active == Some(id) || self.active.is_none() {
            self.active = self.tabs.get(index).or_else(|| self.tabs.last()).copied();
        }
        self.documents.remove(&id)
    }

    /// Activa una pestaña. Devuelve `false` si el documento no existe.
    ///
    /// Con la pantalla dividida, si el documento no está en ningún panel, sustituye al del
    /// panel activo.
    pub fn activate(&mut self, id: DocumentId) -> bool {
        if self.documents.contains_key(&id) {
            self.set_active(id);
            true
        } else {
            false
        }
    }

    /// Mueve una pestaña a la posición `index` (ajustada al número de pestañas).
    pub fn move_tab(&mut self, id: DocumentId, index: usize) {
        let Some(from) = self.tabs.iter().position(|t| *t == id) else {
            return;
        };
        let tab = self.tabs.remove(from);
        let index = index.min(self.tabs.len());
        self.tabs.insert(index, tab);
    }

    // --- Selección múltiple -----------------------------------------------------------

    /// Pestañas seleccionadas, en el orden en que se eligieron.
    pub fn selected(&self) -> &[DocumentId] {
        &self.selected
    }

    pub fn is_selected(&self, id: DocumentId) -> bool {
        self.selected.contains(&id)
    }

    /// Añade o quita una pestaña de la selección (Cmd/Ctrl+clic). Al empezar una
    /// selección, la pestaña activa entra primero.
    pub fn toggle_selection(&mut self, id: DocumentId) {
        if !self.documents.contains_key(&id) {
            return;
        }
        if self.selected.is_empty()
            && let Some(active) = self.active
            && active != id
        {
            self.selected.push(active);
        }
        if let Some(position) = self.selected.iter().position(|s| *s == id) {
            self.selected.remove(position);
        } else {
            self.selected.push(id);
        }
    }

    /// Selecciona las pestañas entre la última elegida (o la activa) e `id` (Mayús+clic).
    pub fn select_range(&mut self, id: DocumentId) {
        let anchor = self.selected.last().copied().or(self.active);
        let position = |target| self.tabs.iter().position(|t| *t == target);
        let (Some(from), Some(to)) = (anchor.and_then(position), position(id)) else {
            return;
        };
        let (start, end) = if from <= to { (from, to) } else { (to, from) };
        self.selected = self
            .tabs
            .get(start..=end)
            .map(<[_]>::to_vec)
            .unwrap_or_default();
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
    }

    // --- Pantalla dividida ------------------------------------------------------------

    /// Documentos en pantalla dividida (vacío si no está dividida).
    pub fn panes(&self) -> &[DocumentId] {
        &self.panes
    }

    pub fn is_split(&self) -> bool {
        !self.panes.is_empty()
    }

    /// Divide la pantalla con los documentos indicados (los [`MAX_PANES`] primeros, sin
    /// repetir). Necesita al menos dos. Limpia la selección.
    pub fn split(&mut self, ids: &[DocumentId]) -> bool {
        let mut panes: Vec<DocumentId> = Vec::with_capacity(MAX_PANES);
        for id in ids {
            if panes.len() < MAX_PANES && self.documents.contains_key(id) && !panes.contains(id) {
                panes.push(*id);
            }
        }
        if panes.len() < 2 {
            return false;
        }
        if !self.active.is_some_and(|a| panes.contains(&a)) {
            self.active = panes.first().copied();
        }
        self.panes = panes;
        self.selected.clear();
        true
    }

    /// Vuelve a una sola vista con el documento activo.
    pub fn unsplit(&mut self) {
        self.panes.clear();
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

    /// Vuelve a leer el archivo con otra codificación, descartando los cambios sin guardar.
    /// Conserva la posición de la pestaña y el lenguaje elegido a mano.
    pub fn reload_with_encoding(
        &mut self,
        id: DocumentId,
        encoding: TextEncoding,
    ) -> Result<(), WorkspaceError> {
        let document = self
            .documents
            .get(&id)
            .ok_or(WorkspaceError::UnknownDocument(id))?;
        let path = document.path().ok_or(WorkspaceError::NoPath)?.to_owned();
        let manual_language = document.is_language_manual().then(|| document.language());
        let bytes = std::fs::read(&path).map_err(|source| WorkspaceError::Read {
            path: path.clone(),
            source,
        })?;
        let decoded = decode_as(&bytes, encoding);
        let line_ending =
            LineEnding::detect(&decoded.text).unwrap_or_else(LineEnding::platform_default);
        let loaded = LoadedText {
            text: LineEnding::normalize(&decoded.text).into_owned(),
            format: TextFormat {
                encoding: decoded.encoding,
                line_ending,
            },
            had_decoding_errors: decoded.had_errors,
        };
        let mut reloaded = Document::from_loaded(id, path, loaded);
        if let Some(language) = manual_language {
            reloaded.set_language(language);
        }
        self.documents.insert(id, reloaded);
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
        self.set_active(id);
        id
    }

    /// Activa un documento; si la pantalla está dividida y no está a la vista, ocupa el
    /// panel activo.
    fn set_active(&mut self, id: DocumentId) {
        if !self.panes.contains(&id)
            && let Some(slot) = self
                .active
                .and_then(|active| self.panes.iter().position(|p| *p == active))
            && let Some(pane) = self.panes.get_mut(slot)
        {
            *pane = id;
        }
        self.active = Some(id);
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
        if let Some(id) = self.tabs.get(next).copied() {
            self.set_active(id);
        }
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
    fn ctrl_click_starts_selection_with_active_tab() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.activate(a);
        ws.toggle_selection(c);
        ws.toggle_selection(b);
        assert_eq!(ws.selected(), &[a, c, b]);
        ws.toggle_selection(c);
        assert_eq!(ws.selected(), &[a, b]);
        ws.clear_selection();
        assert!(ws.selected().is_empty());
    }

    #[test]
    fn shift_click_selects_range_in_tab_order() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        let d = ws.new_document();
        ws.activate(d);
        ws.select_range(b);
        assert_eq!(ws.selected(), &[b, c, d]);
        assert!(ws.is_selected(c) && !ws.is_selected(a));
    }

    #[test]
    fn split_uses_first_four_unique_and_needs_two() {
        let mut ws = Workspace::new();
        let ids: Vec<_> = (0..6).map(|_| ws.new_document()).collect();
        assert!(!ws.split(&[ids[0]]));
        assert!(!ws.is_split());
        let wanted = [ids[5], ids[1], ids[1], ids[3], ids[0], ids[2]];
        assert!(ws.split(&wanted));
        assert_eq!(ws.panes(), &[ids[5], ids[1], ids[3], ids[0]]);
        // El activo (el último creado) está en un panel y se conserva.
        assert_eq!(ws.active_id(), Some(ids[5]));
        ws.unsplit();
        assert!(!ws.is_split());
    }

    #[test]
    fn activating_hidden_tab_replaces_active_pane() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.split(&[a, b]);
        ws.activate(b);
        ws.activate(c);
        assert_eq!(ws.panes(), &[a, c]);
        assert_eq!(ws.active_id(), Some(c));
        // Un documento nuevo también ocupa el panel activo.
        let d = ws.new_document();
        assert_eq!(ws.panes(), &[a, d]);
    }

    #[test]
    fn closing_pane_document_activates_neighbor_and_unsplits_at_one() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.split(&[a, b, c]);
        ws.activate(b);
        ws.close(b);
        assert_eq!(ws.panes(), &[a, c]);
        assert_eq!(ws.active_id(), Some(c));
        ws.close(a);
        assert!(!ws.is_split());
        assert_eq!(ws.active_id(), Some(c));
    }

    #[test]
    fn closing_removes_from_selection() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        ws.toggle_selection(a);
        ws.close(a);
        assert_eq!(ws.selected(), &[b]);
    }

    #[test]
    fn moving_tabs() {
        let mut ws = Workspace::new();
        let a = ws.new_document();
        let b = ws.new_document();
        let c = ws.new_document();
        ws.move_tab(a, 2);
        assert_eq!(ws.tabs(), &[b, c, a]);
        ws.move_tab(a, 0);
        assert_eq!(ws.tabs(), &[a, b, c]);
        ws.move_tab(b, 99);
        assert_eq!(ws.tabs(), &[a, c, b]);
        assert_eq!(ws.active_id(), Some(c));
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
    fn reload_with_other_encoding() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("viejo.txt");
        // UTF-8 válido que en realidad se escribió pensando en otra cosa: forzamos 1252.
        std::fs::write(&path, "canción\r\n".as_bytes()).unwrap();
        let mut ws = Workspace::new();
        let id = ws.open_file(&path).unwrap();
        ws.document_mut(id)
            .unwrap()
            .editor
            .insert_text("cambio")
            .unwrap();
        ws.reload_with_encoding(id, TextEncoding::Windows1252)
            .unwrap();
        let doc = ws.document(id).unwrap();
        assert_eq!(doc.editor.text(), "canciÃ³n\n");
        assert_eq!(doc.format().encoding, TextEncoding::Windows1252);
        assert_eq!(doc.format().line_ending, LineEnding::CrLf);
        assert!(!doc.is_modified());
        assert_eq!(ws.tabs(), &[id]);
    }

    #[test]
    fn reload_needs_a_path() {
        let mut ws = Workspace::new();
        let id = ws.new_document();
        assert!(matches!(
            ws.reload_with_encoding(id, TextEncoding::Utf8),
            Err(WorkspaceError::NoPath)
        ));
    }

    #[test]
    fn open_missing_file_fails() {
        let mut ws = Workspace::new();
        let err = ws.open_file(Path::new("/no/existe/nada.txt")).unwrap_err();
        assert!(matches!(err, WorkspaceError::Read { .. }));
        assert!(ws.is_empty());
    }
}
