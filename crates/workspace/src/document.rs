//! Un documento abierto: estado de edición, ruta y formato en disco.

use std::path::{Path, PathBuf};

use lorca_core::{EditorState, LoadedText, TextFormat};
use lorca_syntax::Language;

use crate::DiskStamp;

/// Identificador estable de un documento mientras está abierto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DocumentId(pub(crate) u64);

impl DocumentId {
    /// Valor numérico, útil para nombres de archivo derivados.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// Documento abierto en una pestaña.
#[derive(Debug, Clone)]
pub struct Document {
    id: DocumentId,
    pub editor: EditorState,
    path: Option<PathBuf>,
    /// Número de los documentos sin título ("Sin título 3"). `None` si tiene ruta.
    untitled_number: Option<u32>,
    format: TextFormat,
    language: Language,
    /// Revisión del historial en el momento de abrir o guardar.
    saved_revision: u64,
    /// Cambios que no pasan por el historial (p. ej. cambiar la codificación).
    forced_modified: bool,
    /// Huella del archivo en disco al abrirlo o guardarlo por última vez.
    disk_stamp: Option<DiskStamp>,
}

impl Document {
    pub(crate) fn untitled(id: DocumentId, number: u32) -> Self {
        Self {
            id,
            editor: EditorState::new(),
            path: None,
            untitled_number: Some(number),
            format: TextFormat::default(),
            language: Language::PlainText,
            saved_revision: 0,
            forced_modified: false,
            disk_stamp: None,
        }
    }

    pub(crate) fn from_loaded(id: DocumentId, path: PathBuf, loaded: LoadedText) -> Self {
        let editor = EditorState::from_text(&loaded.text);
        Self {
            id,
            saved_revision: editor.revision(),
            editor,
            language: Language::from_path(&path),
            disk_stamp: DiskStamp::of(&path),
            path: Some(path),
            untitled_number: None,
            format: loaded.format,
            forced_modified: false,
        }
    }

    /// Documento restaurado de una copia de respaldo: siempre se considera modificado,
    /// porque su contenido no coincide (o puede no coincidir) con el del disco.
    pub(crate) fn restored(
        id: DocumentId,
        path: Option<PathBuf>,
        untitled_number: Option<u32>,
        text: &str,
        format: TextFormat,
        disk_stamp: Option<DiskStamp>,
    ) -> Self {
        let editor = EditorState::from_text(text);
        Self {
            id,
            saved_revision: editor.revision(),
            editor,
            language: path
                .as_deref()
                .map_or(Language::PlainText, Language::from_path),
            untitled_number: if path.is_some() {
                None
            } else {
                untitled_number
            },
            path,
            format,
            forced_modified: true,
            disk_stamp,
        }
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn untitled_number(&self) -> Option<u32> {
        self.untitled_number
    }

    pub fn format(&self) -> TextFormat {
        self.format
    }

    pub fn disk_stamp(&self) -> Option<DiskStamp> {
        self.disk_stamp
    }

    pub fn language(&self) -> Language {
        self.language
    }

    /// Nombre que se muestra en la pestaña: el nombre del archivo o "Sin título N".
    pub fn display_name(&self) -> String {
        match (&self.path, self.untitled_number) {
            (Some(path), _) => path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            (None, Some(n)) => format!("Sin título {n}"),
            (None, None) => "Sin título".to_owned(),
        }
    }

    /// `true` si el contenido difiere de lo que hay en disco (o nunca se ha guardado algo).
    pub fn is_modified(&self) -> bool {
        self.forced_modified || self.editor.revision() != self.saved_revision
    }

    /// Marca el documento como modificado aunque el texto no haya cambiado.
    pub fn mark_modified(&mut self) {
        self.forced_modified = true;
    }

    /// `true` si es un documento sin título, vacío y sin cambios.
    pub fn is_pristine_untitled(&self) -> bool {
        self.path.is_none() && !self.is_modified() && self.editor.buffer().is_empty()
    }

    pub(crate) fn mark_saved(&mut self, path: PathBuf) {
        self.language = Language::from_path(&path);
        self.path = Some(path);
        self.untitled_number = None;
        self.saved_revision = self.editor.revision();
        self.forced_modified = false;
        self.disk_stamp = self.path.as_deref().and_then(DiskStamp::of);
    }
}
