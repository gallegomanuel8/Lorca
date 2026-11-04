//! Un documento abierto: estado de edición, ruta y formato en disco.

use std::path::{Path, PathBuf};

use lorca_core::{EditorState, LoadedText, TextFormat};
use lorca_syntax::Language;

use crate::DiskStamp;

/// Caracteres como máximo del título de un documento sin título tomado de su texto.
const TITLE_CHARS: usize = 30;

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
    /// `true` si el usuario eligió el lenguaje a mano: ya no se detecta automáticamente.
    language_manual: bool,
    /// `true` si el lenguaje se dedujo del texto pegado: se conserva aunque la ruta no lo
    /// indique (hasta que el documento quede vacío).
    language_guessed: bool,
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
            language_manual: false,
            language_guessed: false,
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
            language: Language::detect(Some(&path), &editor.buffer().line_text(0)),
            language_manual: false,
            language_guessed: false,
            editor,
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
            language: Language::detect(path.as_deref(), &editor.buffer().line_text(0)),
            language_manual: false,
            language_guessed: false,
            editor,
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

    pub fn is_language_manual(&self) -> bool {
        self.language_manual
    }

    /// Fija el lenguaje a mano; a partir de ahora no se vuelve a detectar.
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        self.language_manual = true;
    }

    pub fn is_language_guessed(&self) -> bool {
        self.language_guessed
    }

    /// Vuelve a detectar el lenguaje (ruta y shebang) salvo que se haya elegido a mano.
    /// Un lenguaje deducido al pegar se mantiene si la ruta y el shebang no dicen otro y
    /// el documento no está vacío. Es barato: solo mira la ruta y la primera línea.
    pub fn refresh_language(&mut self) {
        if self.language_manual {
            return;
        }
        let detected = Language::detect(self.path.as_deref(), &self.editor.buffer().line_text(0));
        if self.language_guessed
            && detected == Language::PlainText
            && !self.editor.buffer().is_empty()
        {
            return;
        }
        self.language = detected;
        self.language_guessed = false;
    }

    /// Si el documento es texto plano (no elegido a mano), deduce el lenguaje del texto
    /// pegado. Devuelve el lenguaje nuevo si lo ha cambiado.
    pub fn guess_language_from_paste(&mut self, pasted: &str) -> Option<Language> {
        if self.language_manual || self.language != Language::PlainText {
            return None;
        }
        let guessed = Language::guess(pasted);
        if guessed == Language::PlainText {
            return None;
        }
        self.set_guessed_language(guessed);
        Some(guessed)
    }

    /// Al dar formato con marcas de Markdown (negrita, listas…) a un documento de texto
    /// plano, pasa a tratarse como Markdown (como un lenguaje deducido: se mantiene aunque
    /// la ruta sea `.txt`). No cambia un lenguaje elegido a mano.
    pub fn adopt_markdown(&mut self) {
        if !self.language_manual && self.language == Language::PlainText {
            self.set_guessed_language(Language::Markdown);
        }
    }

    /// Fija un lenguaje deducido (al pegar o al restaurar la sesión).
    pub(crate) fn set_guessed_language(&mut self, language: Language) {
        self.language = language;
        self.language_guessed = true;
    }

    /// Nombre que se muestra en la pestaña: el nombre del archivo o, si no lo tiene, el
    /// comienzo del texto (como el Bloc de notas); vacío, "Sin título N".
    pub fn display_name(&self) -> String {
        match &self.path {
            Some(path) => path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            ),
            None => self.text_title().unwrap_or_else(|| self.untitled_label()),
        }
    }

    /// Nombre de archivo que se propone al guardar por primera vez: el comienzo del texto
    /// sin los caracteres que no admiten los sistemas de archivos, con `.txt`.
    pub fn suggested_file_name(&self) -> String {
        if self.path.is_some() {
            return self.display_name();
        }
        let base = self
            .text_title()
            .map(|title| {
                title
                    .trim_end_matches('…')
                    .chars()
                    .map(|c| match c {
                        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
                        c if c.is_control() => ' ',
                        c => c,
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim_end_matches('.')
                    .to_owned()
            })
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.untitled_label());
        format!("{base}.txt")
    }

    fn untitled_label(&self) -> String {
        match self.untitled_number {
            Some(n) => format!("Sin título {n}"),
            None => "Sin título".to_owned(),
        }
    }

    /// Primera línea con texto (sin las almohadillas de un título de Markdown y con los
    /// espacios seguidos reducidos a uno), cortada a [`TITLE_CHARS`] caracteres.
    fn text_title(&self) -> Option<String> {
        /// Líneas que se miran como mucho buscando una con texto.
        const LOOKAHEAD: usize = 50;
        let buffer = self.editor.buffer();
        let line = (0..buffer.len_lines().min(LOOKAHEAD))
            .map(|line| buffer.line_text(line))
            .find(|text| !text.trim().is_empty())?;
        let words = line
            .trim_start()
            .trim_start_matches('#')
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if words.is_empty() {
            return None;
        }
        if words.chars().count() <= TITLE_CHARS {
            return Some(words);
        }
        let mut title: String = words.chars().take(TITLE_CHARS).collect();
        title.truncate(title.trim_end().len());
        title.push('…');
        Some(title)
    }

    /// `true` si el contenido difiere de lo que hay en disco (o nunca se ha guardado algo).
    pub fn is_modified(&self) -> bool {
        self.forced_modified || self.editor.revision() != self.saved_revision
    }

    /// Cambia la codificación o el final de línea con que se guardará. Si cambia algo, el
    /// documento queda modificado (el archivo en disco aún tiene el formato anterior).
    pub fn set_format(&mut self, format: TextFormat) {
        if format != self.format {
            self.format = format;
            self.forced_modified = true;
        }
    }

    /// Acepta la versión actual del disco como base (p. ej. cuando el usuario decide
    /// conservar sus cambios tras una modificación externa), para no volver a avisar.
    pub fn acknowledge_disk_change(&mut self) {
        self.disk_stamp = self.path.as_deref().and_then(DiskStamp::of);
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
        self.path = Some(path);
        self.refresh_language();
        self.untitled_number = None;
        self.saved_revision = self.editor.revision();
        self.forced_modified = false;
        self.disk_stamp = self.path.as_deref().and_then(DiskStamp::of);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn untitled_detects_shebang_when_refreshed() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        doc.editor
            .insert_text("#!/usr/bin/env python3\nprint(1)")
            .unwrap();
        assert_eq!(doc.language(), Language::PlainText);
        doc.refresh_language();
        assert_eq!(doc.language(), Language::Python);
    }

    #[test]
    fn untitled_documents_are_named_after_their_text() {
        let mut doc = Document::untitled(DocumentId(1), 3);
        assert_eq!(doc.display_name(), "Sin título 3");
        assert_eq!(doc.suggested_file_name(), "Sin título 3.txt");
        doc.editor
            .insert_text("\n   \n## Lista   de la\tcompra\n- pan")
            .unwrap();
        assert_eq!(doc.display_name(), "Lista de la compra");
        doc.editor.select_all();
        doc.editor
            .insert_text("Verde que te quiero verde. Verde viento. Verdes ramas.")
            .unwrap();
        assert_eq!(doc.display_name(), "Verde que te quiero verde. Ver…");
        doc.editor.select_all();
        doc.editor.insert_text("¿Notas: a/b?").unwrap();
        assert_eq!(doc.suggested_file_name(), "¿Notas a b.txt");
        // Solo almohadillas o espacios: no hay título.
        doc.editor.select_all();
        doc.editor.insert_text("###").unwrap();
        assert_eq!(doc.display_name(), "Sin título 3");
    }

    #[test]
    fn pasted_text_sets_language_until_emptied() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        doc.editor.insert_text("{\"a\": 1}").unwrap();
        doc.refresh_language();
        assert_eq!(
            doc.guess_language_from_paste("{\"a\": 1}"),
            Some(Language::Json)
        );
        assert_eq!(doc.language(), Language::Json);
        // Seguir editando no lo devuelve a texto plano.
        doc.editor.insert_text(" ").unwrap();
        doc.refresh_language();
        assert_eq!(doc.language(), Language::Json);
        // Ya tiene lenguaje: otro pegado no lo cambia.
        assert_eq!(doc.guess_language_from_paste("SELECT * FROM t;"), None);
        // Vacío, vuelve a no tener lenguaje.
        doc.editor.select_all();
        doc.editor.backspace().unwrap();
        doc.refresh_language();
        assert_eq!(doc.language(), Language::PlainText);
        assert!(!doc.is_language_guessed());
    }

    #[test]
    fn formatting_turns_plain_text_into_markdown() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        doc.adopt_markdown();
        assert_eq!(doc.language(), Language::Markdown);
        doc.editor.insert_text("**hola**").unwrap();
        doc.refresh_language();
        assert_eq!(doc.language(), Language::Markdown);
        let mut manual = Document::untitled(DocumentId(2), 2);
        manual.set_language(Language::PlainText);
        manual.adopt_markdown();
        assert_eq!(manual.language(), Language::PlainText);
    }

    #[test]
    fn pasting_respects_manual_language() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        doc.set_language(Language::PlainText);
        assert_eq!(doc.guess_language_from_paste("{\"a\": 1}"), None);
        assert_eq!(doc.language(), Language::PlainText);
    }

    #[test]
    fn manual_language_is_kept() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        doc.set_language(Language::Sql);
        doc.editor.insert_text("#!/bin/bash").unwrap();
        doc.refresh_language();
        assert_eq!(doc.language(), Language::Sql);
        doc.mark_saved(PathBuf::from("/x/script.py"));
        assert_eq!(doc.language(), Language::Sql);
        assert!(doc.is_language_manual());
    }

    #[test]
    fn changing_format_marks_modified() {
        let mut doc = Document::untitled(DocumentId(1), 1);
        let format = doc.format();
        doc.set_format(format);
        assert!(!doc.is_modified());
        let crlf = TextFormat {
            line_ending: lorca_core::LineEnding::CrLf,
            ..format
        };
        let lf = TextFormat {
            line_ending: lorca_core::LineEnding::Lf,
            ..format
        };
        doc.set_format(if format.line_ending == lorca_core::LineEnding::Lf {
            crlf
        } else {
            lf
        });
        assert!(doc.is_modified());
    }

    #[test]
    fn loaded_file_without_extension_uses_shebang() {
        let loaded = lorca_core::load_bytes(b"#!/bin/sh\necho hola\n");
        let doc = Document::from_loaded(DocumentId(1), PathBuf::from("/x/deploy"), loaded);
        assert_eq!(doc.language(), Language::Bash);
    }
}
