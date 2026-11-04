//! Sesión: pestañas abiertas y cambios sin guardar que sobreviven al cierre de la app.
//!
//! Se guarda en `session.toml` dentro del directorio de datos. El texto de los documentos
//! con cambios sin guardar (y de los «Sin título» con contenido) va a `backups/`, en
//! UTF-8 y con finales LF; los archivos originales nunca se tocan.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lorca_core::{LineEnding, Selection, TextEncoding, TextFormat, load_bytes};
use lorca_syntax::Language;
use serde::{Deserialize, Serialize};

use crate::storage::{read_optional, write_atomic};
use crate::{DiskStamp, Document, DocumentId, Workspace, WorkspaceError};

/// Versión del formato de `session.toml`.
pub const SESSION_VERSION: u32 = 1;

/// Contenido de `session.toml`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub version: u32,
    /// Índice de la pestaña activa.
    pub active: Option<usize>,
    #[serde(default)]
    pub documents: Vec<SessionDocument>,
    /// Índices (en `documents`) de los paneles de la pantalla dividida.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub split: Vec<usize>,
}

/// Estado guardado de una pestaña.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDocument {
    pub path: Option<PathBuf>,
    pub untitled_number: Option<u32>,
    /// Nombre del archivo de respaldo con el texto sin guardar, si lo hay.
    pub backup: Option<String>,
    pub encoding: String,
    pub line_ending: String,
    pub anchor: usize,
    pub head: usize,
    pub scroll_x: f32,
    pub scroll_y: f32,
    /// Lenguaje elegido a mano (identificador de `Language::id`), si lo hay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// `true` si `language` se dedujo al pegar (no se eligió a mano).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub language_guessed: bool,
    /// Huella del archivo en disco en el que se basan los cambios sin guardar.
    pub disk_stamp: Option<DiskStamp>,
}

/// Resultado de restaurar una sesión.
#[derive(Debug, Default)]
pub struct RestoreReport {
    /// Desplazamiento guardado de cada documento restaurado.
    pub scrolls: Vec<(DocumentId, f32, f32)>,
    /// Avisos para el usuario (archivos que faltan o han cambiado en disco).
    pub warnings: Vec<String>,
}

/// Lee y escribe la sesión en un directorio.
#[derive(Debug)]
pub struct SessionStore {
    dir: PathBuf,
    /// Revisión ya escrita de cada respaldo, para no reescribir textos sin cambios.
    written: HashMap<DocumentId, u64>,
}

impl SessionStore {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            written: HashMap::new(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn session_path(&self) -> PathBuf {
        self.dir.join("session.toml")
    }

    fn backups_dir(&self) -> PathBuf {
        self.dir.join("backups")
    }

    /// Lee la sesión guardada; `None` si no hay ninguna.
    pub fn load(&self) -> Result<Option<Session>, WorkspaceError> {
        let path = self.session_path();
        let Some(contents) = read_optional(&path)? else {
            return Ok(None);
        };
        toml::from_str(&contents)
            .map(Some)
            .map_err(|e| WorkspaceError::Parse {
                path,
                message: e.to_string(),
            })
    }

    /// Lee el texto de un respaldo.
    pub fn read_backup(&self, name: &str) -> Result<String, WorkspaceError> {
        let path = self.backups_dir().join(name);
        std::fs::read_to_string(&path).map_err(|source| WorkspaceError::Read { path, source })
    }

    /// Guarda la sesión del workspace. `scroll` da el desplazamiento de cada documento.
    pub fn save(
        &mut self,
        workspace: &Workspace,
        scroll: impl Fn(DocumentId) -> (f32, f32),
    ) -> Result<(), WorkspaceError> {
        let backups_dir = self.backups_dir();
        let mut session = Session {
            version: SESSION_VERSION,
            active: workspace
                .active_id()
                .and_then(|id| workspace.tabs().iter().position(|t| *t == id)),
            documents: Vec::with_capacity(workspace.len()),
            split: workspace
                .panes()
                .iter()
                .filter_map(|id| workspace.tabs().iter().position(|t| t == id))
                .collect(),
        };
        let mut kept = HashSet::new();

        for document in workspace.documents() {
            let id = document.id();
            let backup = if document.is_modified() {
                let name = format!("doc-{}.txt", id.get());
                let revision = document.editor.version();
                let path = backups_dir.join(&name);
                if self.written.get(&id) != Some(&revision) || !path.exists() {
                    write_atomic(&path, document.editor.text().as_bytes())?;
                    self.written.insert(id, revision);
                }
                kept.insert(name.clone());
                Some(name)
            } else {
                self.written.remove(&id);
                None
            };
            let (scroll_x, scroll_y) = scroll(id);
            let selection = document.editor.selection();
            let format = document.format();
            session.documents.push(SessionDocument {
                path: document.path().map(Path::to_path_buf),
                untitled_number: document.untitled_number(),
                backup,
                encoding: format.encoding.id().to_owned(),
                line_ending: format.line_ending.id().to_owned(),
                anchor: selection.anchor,
                head: selection.head,
                scroll_x,
                scroll_y,
                language: (document.is_language_manual() || document.is_language_guessed())
                    .then(|| document.language().id().to_owned()),
                language_guessed: document.is_language_guessed(),
                disk_stamp: document.disk_stamp(),
            });
        }

        let path = self.session_path();
        let contents = toml::to_string_pretty(&session).map_err(|e| WorkspaceError::Serialize {
            path: path.clone(),
            message: e.to_string(),
        })?;
        write_atomic(&path, contents.as_bytes())?;

        self.written
            .retain(|id, _| workspace.document(*id).is_some());
        remove_stale_backups(&backups_dir, &kept);
        Ok(())
    }
}

/// Borra los respaldos que ya no usa ninguna pestaña. Los fallos se ignoran: un respaldo
/// huérfano solo ocupa espacio y se volverá a intentar en el siguiente guardado.
fn remove_stale_backups(dir: &Path, kept: &HashSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !kept.contains(&name) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

impl Workspace {
    /// Restaura las pestañas de una sesión, añadiéndolas a las ya abiertas.
    ///
    /// `read_backup` obtiene el texto de un respaldo por su nombre.
    pub fn restore(
        &mut self,
        session: &Session,
        read_backup: impl Fn(&str) -> Result<String, WorkspaceError>,
    ) -> RestoreReport {
        let mut report = RestoreReport::default();
        // Id restaurado de cada documento de la sesión (None si no se pudo restaurar).
        let mut ids = Vec::with_capacity(session.documents.len());

        for saved in &session.documents {
            let id = self.restore_document(saved, &read_backup, &mut report.warnings);
            if let Some(id) = id {
                if let Some(document) = self.document_mut(id) {
                    document
                        .editor
                        .set_selection(Selection::new(saved.anchor, saved.head));
                    match saved.language.as_deref().and_then(Language::from_id) {
                        Some(language) if saved.language_guessed => {
                            document.set_guessed_language(language);
                        }
                        Some(language) => document.set_language(language),
                        None => {}
                    }
                }
                report.scrolls.push((id, saved.scroll_x, saved.scroll_y));
            }
            ids.push(id);
        }

        let panes: Vec<DocumentId> = session
            .split
            .iter()
            .filter_map(|index| ids.get(*index).copied().flatten())
            .collect();
        self.split(&panes);

        // Si la pestaña activa no se pudo restaurar, se activa la última restaurada.
        let active = session
            .active
            .and_then(|index| ids.get(index).copied().flatten());
        if let Some(id) = active.or_else(|| ids.iter().flatten().last().copied()) {
            self.activate(id);
        }
        report
    }

    fn restore_document(
        &mut self,
        saved: &SessionDocument,
        read_backup: &impl Fn(&str) -> Result<String, WorkspaceError>,
        warnings: &mut Vec<String>,
    ) -> Option<DocumentId> {
        let format = TextFormat {
            encoding: TextEncoding::from_id(&saved.encoding).unwrap_or_default(),
            line_ending: LineEnding::from_id(&saved.line_ending)
                .unwrap_or_else(LineEnding::platform_default),
        };
        let name = || {
            saved.path.as_deref().map_or_else(
                || format!("Sin título {}", saved.untitled_number.unwrap_or(1)),
                |p| p.display().to_string(),
            )
        };

        let backup_text = match saved.backup.as_deref().map(read_backup) {
            Some(Ok(text)) => Some(text),
            Some(Err(error)) => {
                warnings.push(format!(
                    "No se pudo recuperar el respaldo de «{}»: {error}",
                    name()
                ));
                None
            }
            None => None,
        };

        match (&saved.path, backup_text) {
            (path, Some(text)) => {
                if let Some(path) = path {
                    match DiskStamp::of(path) {
                        None => warnings.push(format!(
                            "«{}» ya no existe en disco; se mantiene tu versión sin guardar",
                            path.display()
                        )),
                        Some(now) if Some(now) != saved.disk_stamp => warnings.push(format!(
                            "«{}» cambió en disco mientras Lorca estaba cerrado; se mantiene tu \
                             versión sin guardar",
                            path.display()
                        )),
                        Some(_) => {}
                    }
                }
                Some(self.push_document(|id| {
                    Document::restored(
                        id,
                        path.clone(),
                        saved.untitled_number,
                        &text,
                        format,
                        saved.disk_stamp,
                    )
                }))
            }
            (Some(path), None) => match std::fs::read(path) {
                Ok(bytes) => {
                    let loaded = load_bytes(&bytes);
                    Some(self.push_document(|id| Document::from_loaded(id, path.clone(), loaded)))
                }
                Err(error) => {
                    warnings.push(format!("No se pudo reabrir «{}»: {error}", path.display()));
                    None
                }
            },
            (None, None) if saved.backup.is_none() => {
                let number = saved.untitled_number.unwrap_or(1);
                Some(self.push_document(|id| Document::untitled(id, number)))
            }
            // Un «Sin título» cuyo respaldo se perdió no tiene nada que restaurar.
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_core::Motion;

    fn scroll_of(_: DocumentId) -> (f32, f32) {
        (0.0, 40.0)
    }

    /// Guarda la sesión de `ws` en `dir` y la restaura en un workspace nuevo.
    fn save_and_restore(ws: &Workspace, dir: &Path) -> (Workspace, RestoreReport) {
        let mut store = SessionStore::new(dir.to_owned());
        store.save(ws, scroll_of).unwrap();
        let session = store.load().unwrap().unwrap();
        let mut restored = Workspace::new();
        let report = restored.restore(&session, |name| store.read_backup(name));
        (restored, report)
    }

    #[test]
    fn tab_order_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("archivo.txt");
        std::fs::write(&file, "en disco").unwrap();
        let mut ws = Workspace::new();
        let a = ws.new_document();
        ws.active_mut().unwrap().editor.insert_text("uno").unwrap();
        ws.open_file(&file).unwrap();
        let c = ws.new_document();
        ws.active_mut().unwrap().editor.insert_text("tres").unwrap();
        ws.move_tab(c, 0);
        ws.activate(a);
        let before: Vec<String> = ws.documents().map(|d| d.editor.text()).collect();
        let (restored, _) = save_and_restore(&ws, dir.path());
        let after: Vec<String> = restored.documents().map(|d| d.editor.text()).collect();
        assert_eq!(after, before);
    }

    #[test]
    fn no_session_file_means_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            SessionStore::new(dir.path().to_owned())
                .load()
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn restores_untitled_with_unsaved_text_and_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        ws.new_document();
        let doc = ws.active_mut().unwrap();
        doc.editor.insert_text("borrador\nde poema").unwrap();
        doc.editor.move_cursor(Motion::Left, true);

        let (restored, report) = save_and_restore(&ws, dir.path());
        let doc = restored.active().unwrap();
        assert_eq!(doc.editor.text(), "borrador\nde poema");
        assert_eq!(doc.untitled_number(), Some(1));
        assert_eq!(doc.display_name(), "borrador");
        assert!(doc.is_modified());
        assert_eq!(doc.editor.selection(), Selection::new(17, 16));
        assert_eq!(report.scrolls, vec![(doc.id(), 0.0, 40.0)]);
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn restores_tab_order_active_tab_and_empty_untitled() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("romance.md");
        std::fs::write(&file, "verde").unwrap();

        let mut ws = Workspace::new();
        ws.new_document();
        let second = ws.new_document();
        ws.open_file(&file).unwrap();
        ws.activate(second);

        let (restored, _) = save_and_restore(&ws, dir.path());
        let names: Vec<_> = restored.documents().map(Document::display_name).collect();
        assert_eq!(names, ["Sin título 1", "Sin título 2", "romance.md"]);
        assert_eq!(restored.active().unwrap().display_name(), "Sin título 2");
        assert!(!restored.has_unsaved_changes());
    }

    #[test]
    fn unmodified_file_is_reloaded_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "antes").unwrap();
        let mut ws = Workspace::new();
        ws.open_file(&file).unwrap();
        let mut store = SessionStore::new(dir.path().join("datos"));
        store.save(&ws, scroll_of).unwrap();

        std::fs::write(&file, "después").unwrap();
        let session = store.load().unwrap().unwrap();
        let mut restored = Workspace::new();
        let report = restored.restore(&session, |n| store.read_backup(n));
        assert_eq!(restored.active().unwrap().editor.text(), "después");
        assert!(report.warnings.is_empty());
    }

    #[test]
    fn modified_file_keeps_user_version_and_warns_if_disk_changed() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "original\r\n").unwrap();
        let mut ws = Workspace::new();
        ws.open_file(&file).unwrap();
        let doc = ws.active_mut().unwrap();
        doc.editor.move_cursor(Motion::DocumentEnd, false);
        doc.editor.insert_text("mío").unwrap();
        let mut store = SessionStore::new(dir.path().join("datos"));
        store.save(&ws, scroll_of).unwrap();

        std::fs::write(&file, "otra cosa, más larga").unwrap();
        let session = store.load().unwrap().unwrap();
        let mut restored = Workspace::new();
        let report = restored.restore(&session, |n| store.read_backup(n));

        let doc = restored.active().unwrap();
        assert_eq!(doc.editor.text(), "original\nmío");
        assert!(doc.is_modified());
        assert_eq!(doc.format().line_ending, LineEnding::CrLf);
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings[0].contains("cambió en disco"));
    }

    #[test]
    fn modified_file_deleted_from_disk_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let mut ws = Workspace::new();
        ws.open_file(&file).unwrap();
        ws.active_mut().unwrap().editor.insert_text("y").unwrap();
        let mut store = SessionStore::new(dir.path().join("datos"));
        store.save(&ws, scroll_of).unwrap();

        std::fs::remove_file(&file).unwrap();
        let session = store.load().unwrap().unwrap();
        let mut restored = Workspace::new();
        let report = restored.restore(&session, |n| store.read_backup(n));
        assert_eq!(restored.active().unwrap().editor.text(), "yx");
        assert!(report.warnings[0].contains("ya no existe"));
    }

    #[test]
    fn unmodified_file_deleted_from_disk_is_skipped_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "x").unwrap();
        let mut ws = Workspace::new();
        ws.open_file(&file).unwrap();
        let mut store = SessionStore::new(dir.path().join("datos"));
        store.save(&ws, scroll_of).unwrap();

        std::fs::remove_file(&file).unwrap();
        let session = store.load().unwrap().unwrap();
        let mut restored = Workspace::new();
        let report = restored.restore(&session, |n| store.read_backup(n));
        assert!(restored.is_empty());
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn split_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        let a = ws.new_document();
        ws.new_document();
        let c = ws.new_document();
        ws.split(&[c, a]);
        let (restored, _) = save_and_restore(&ws, dir.path());
        let names: Vec<_> = restored
            .panes()
            .iter()
            .map(|id| restored.document(*id).unwrap().display_name())
            .collect();
        assert_eq!(names, ["Sin título 3", "Sin título 1"]);
        assert_eq!(restored.active().unwrap().display_name(), "Sin título 3");
    }

    #[test]
    fn guessed_language_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        ws.new_document();
        let doc = ws.active_mut().unwrap();
        doc.editor.insert_text("SELECT * FROM t;").unwrap();
        doc.guess_language_from_paste("SELECT * FROM t;");
        let (restored, _) = save_and_restore(&ws, dir.path());
        let doc = restored.active().unwrap();
        assert_eq!(doc.language(), Language::Sql);
        assert!(doc.is_language_guessed());
        assert!(!doc.is_language_manual());
    }

    #[test]
    fn manual_language_survives_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        ws.new_document();
        ws.active_mut().unwrap().set_language(Language::Sql);
        let (restored, _) = save_and_restore(&ws, dir.path());
        let doc = restored.active().unwrap();
        assert_eq!(doc.language(), Language::Sql);
        assert!(doc.is_language_manual());
    }

    #[test]
    fn backup_follows_typing_within_the_same_word() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        ws.new_document();
        ws.active_mut().unwrap().editor.insert_text("a").unwrap();
        let mut store = SessionStore::new(dir.path().to_owned());
        store.save(&ws, scroll_of).unwrap();
        // «b» se agrupa con «a» en la misma transacción de deshacer.
        ws.active_mut().unwrap().editor.insert_text("b").unwrap();
        store.save(&ws, scroll_of).unwrap();
        let session = store.load().unwrap().unwrap();
        let name = session.documents[0].backup.clone().unwrap();
        assert_eq!(store.read_backup(&name).unwrap(), "ab");
    }

    #[test]
    fn stale_backups_are_removed() {
        let dir = tempfile::tempdir().unwrap();
        let mut ws = Workspace::new();
        let id = ws.new_document();
        ws.active_mut().unwrap().editor.insert_text("x").unwrap();
        let mut store = SessionStore::new(dir.path().to_owned());
        store.save(&ws, scroll_of).unwrap();
        assert_eq!(
            std::fs::read_dir(dir.path().join("backups"))
                .unwrap()
                .count(),
            1
        );

        ws.close(id);
        store.save(&ws, scroll_of).unwrap();
        assert_eq!(
            std::fs::read_dir(dir.path().join("backups"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn saved_document_drops_its_backup() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        let mut ws = Workspace::new();
        let id = ws.new_document();
        ws.active_mut().unwrap().editor.insert_text("x").unwrap();
        let mut store = SessionStore::new(dir.path().join("datos"));
        store.save(&ws, scroll_of).unwrap();
        ws.save_as(id, &file).unwrap();
        store.save(&ws, scroll_of).unwrap();

        let session = store.load().unwrap().unwrap();
        assert_eq!(session.documents[0].backup, None);
        assert_eq!(session.documents[0].untitled_number, None);
    }

    #[test]
    fn missing_backup_of_untitled_is_skipped() {
        let session = Session {
            version: SESSION_VERSION,
            active: Some(0),
            split: Vec::new(),
            documents: vec![SessionDocument {
                path: None,
                untitled_number: Some(3),
                backup: Some("doc-1.txt".into()),
                encoding: "utf-8".into(),
                line_ending: "lf".into(),
                anchor: 0,
                head: 0,
                scroll_x: 0.0,
                scroll_y: 0.0,
                language: None,
                language_guessed: false,
                disk_stamp: None,
            }],
        };
        let mut ws = Workspace::new();
        let report = ws.restore(&session, |_| Err(WorkspaceError::NoPath));
        assert!(ws.is_empty());
        assert_eq!(report.warnings.len(), 1);
    }
}
