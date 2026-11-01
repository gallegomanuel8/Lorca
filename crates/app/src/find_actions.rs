//! Acciones de búsqueda, sustitución, marcas, marcadores e «Ir a línea» sobre `Lorca`.
//!
//! Es un submódulo de `app` para poder usar su estado privado.

use std::ops::Range;
use std::sync::Arc;

use iced::Task;
use iced::widget::operation;
use lorca_core::{Position, Selection};
use lorca_search::{SearchOptions, SearchQuery, match_index, next_match, previous_match};
use lorca_workspace::DocumentId;

use super::{Lorca, Message};
use crate::find::{
    self, FindMessage, MAX_MATCHES, ResultGroup, SearchResults, build_hits, parse_goto,
};

/// Qué control recibe el teclado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    Editor,
    FindBar,
    GoTo,
}

/// Coincidencias calculadas para un documento, una revisión y una consulta.
#[derive(Debug, Clone)]
pub struct MatchCache {
    id: DocumentId,
    revision: u64,
    pattern: String,
    options: SearchOptions,
    scope: Option<Range<usize>>,
    pub matches: Arc<Vec<Range<usize>>>,
}

impl MatchCache {
    fn is_for(
        &self,
        id: DocumentId,
        revision: u64,
        pattern: &str,
        options: SearchOptions,
        scope: &Option<Range<usize>>,
    ) -> bool {
        self.id == id
            && self.revision == revision
            && self.pattern == pattern
            && self.options == options
            && &self.scope == scope
    }
}

impl Lorca {
    /// Consulta de la barra de búsqueda. Guarda el error (si lo hay) para mostrarlo.
    fn find_query(&mut self) -> Option<SearchQuery> {
        if self.find.pattern.is_empty() {
            self.find.error = None;
            return None;
        }
        match SearchQuery::new(&self.find.pattern, self.find.options) {
            Ok(query) => {
                self.find.error = None;
                Some(query)
            }
            Err(error) => {
                self.find.error = Some(error.to_string());
                None
            }
        }
    }

    /// Coincidencias de la búsqueda actual en el documento activo (con caché).
    pub(super) fn find_matches(&mut self) -> Option<Arc<Vec<Range<usize>>>> {
        let query = self.find_query()?;
        let document = self.workspace.active()?;
        let (id, revision) = (document.id(), document.editor.revision());
        let scope = self.find.scope.clone();
        if let Some(cache) = &self.find_cache
            && cache.is_for(id, revision, &self.find.pattern, self.find.options, &scope)
        {
            return Some(cache.matches.clone());
        }
        let mut matches = query.find_all(&document.editor.text(), MAX_MATCHES);
        if let Some(scope) = &scope {
            matches.retain(|m| m.start >= scope.start && m.end <= scope.end);
        }
        let matches = Arc::new(matches);
        self.find_cache = Some(MatchCache {
            id,
            revision,
            pattern: self.find.pattern.clone(),
            options: self.find.options,
            scope,
            matches: matches.clone(),
        });
        Some(matches)
    }

    /// Recalcula lo que se pinta: coincidencias (si la barra está abierta) y marcas.
    pub(super) fn refresh_search(&mut self) {
        if self.find.open {
            if self.find_matches().is_none() {
                self.find_cache = None;
            }
        } else {
            self.find_cache = None;
        }

        let Some(document) = self.workspace.active() else {
            return;
        };
        let (id, revision) = (document.id(), document.editor.revision());
        let Some(query) = self.marks.get(&id) else {
            self.mark_cache.remove(&id);
            return;
        };
        let fresh = self.mark_cache.get(&id).is_some_and(|cache| {
            cache.is_for(id, revision, query.pattern(), query.options(), &None)
        });
        if !fresh {
            let matches = Arc::new(query.find_all(&document.editor.text(), MAX_MATCHES));
            self.mark_cache.insert(
                id,
                MatchCache {
                    id,
                    revision,
                    pattern: query.pattern().to_owned(),
                    options: query.options(),
                    scope: None,
                    matches,
                },
            );
        }
    }

    /// Texto del contador de la barra: «3 de 17», «17 coincidencias»…
    pub(super) fn find_counter(&self) -> String {
        let Some(cache) = &self.find_cache else {
            return String::new();
        };
        let total = cache.matches.len();
        let plus = if total >= MAX_MATCHES { "+" } else { "" };
        let selection = self
            .workspace
            .active()
            .map(|d| d.editor.selection().range());
        match selection.and_then(|s| match_index(&cache.matches, &s)) {
            Some(index) => format!("{} de {total}{plus}", index + 1),
            None if total == 0 => "Sin coincidencias".to_owned(),
            None if total == 1 => "1 coincidencia".to_owned(),
            None => format!("{total}{plus} coincidencias"),
        }
    }

    /// Selecciona la coincidencia siguiente o anterior.
    fn find_step(&mut self, forward: bool) {
        let Some(matches) = self.find_matches() else {
            if self.find.error.is_none() {
                self.find.status = Some("Escribe algo para buscar".to_owned());
            }
            return;
        };
        let wrap = self.find.wrap;
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let selection = document.editor.selection().range();
        let found = if forward {
            next_match(&matches, &selection, wrap)
        } else {
            previous_match(&matches, &selection, wrap)
        };
        let Some(range) = found.and_then(|index| matches.get(index)).cloned() else {
            self.find.status = Some(format!("No se encontró «{}»", self.find.pattern));
            return;
        };
        let wrapped = if forward {
            range.start < selection.start
        } else {
            range.start > selection.start
        };
        self.find.status = wrapped.then(|| "Se dio la vuelta al documento".to_owned());
        document
            .editor
            .set_selection(Selection::new(range.start, range.end));
        self.reveal_active_cursor();
    }

    /// Abre la barra de búsqueda (o de sustitución) con el foco en el campo indicado.
    pub(super) fn open_find(&mut self, field: &'static str) -> Task<Message> {
        self.goto = None;
        self.find.open = true;
        self.find.status = None;
        self.focus = Focus::FindBar;
        // Como en Notepad++, una selección de una sola línea se usa como texto a buscar.
        if let Some(selected) = self
            .workspace
            .active()
            .and_then(|d| d.editor.selected_text())
            && !selected.contains('\n')
            && selected.chars().count() <= 200
        {
            self.find.pattern = selected;
        }
        Task::batch([operation::focus(field), operation::select_all(field)])
    }

    pub(super) fn find_next(&mut self, forward: bool) -> Task<Message> {
        if self.find.pattern.is_empty() {
            return self.open_find(find::PATTERN_ID);
        }
        self.find_step(forward);
        Task::none()
    }

    /// Busca la palabra bajo el cursor (o la selección) sin abrir la barra.
    pub(super) fn find_word_under_cursor(&mut self) {
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let Some(word) = select_word_near_cursor(&mut document.editor) else {
            return;
        };
        self.find.pattern = word;
        self.find.options.mode = lorca_search::SearchMode::Normal;
        self.find_step(true);
    }

    pub(super) fn open_goto(&mut self) -> Task<Message> {
        self.goto = Some(String::new());
        self.focus = Focus::GoTo;
        operation::focus(find::GOTO_ID)
    }

    /// Escape: cierra «Ir a línea» o la barra de búsqueda y devuelve el foco al editor.
    pub(super) fn escape(&mut self) {
        if self.goto.take().is_none() && self.find.open {
            self.find.open = false;
            self.find.status = None;
        }
        self.focus = Focus::Editor;
    }

    pub(super) fn bookmark_action(&mut self, action: BookmarkAction) {
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        match action {
            BookmarkAction::Toggle => {
                document.editor.toggle_bookmark();
            }
            BookmarkAction::Next | BookmarkAction::Previous => {
                if document
                    .editor
                    .go_to_bookmark(action == BookmarkAction::Next)
                {
                    self.reveal_active_cursor();
                } else {
                    self.find.status = Some("No hay marcadores".to_owned());
                }
            }
        }
    }

    fn reveal_active_cursor(&mut self) {
        let Some(document) = self.workspace.active() else {
            return;
        };
        let view = self.views.entry(document.id()).or_default();
        view.jump_to_cursor(&document.editor, view.viewport);
    }

    pub(super) fn handle_find(&mut self, message: FindMessage) -> Task<Message> {
        match message {
            FindMessage::PatternChanged(pattern) => {
                self.find.pattern = pattern;
                self.find.status = None;
                self.focus = Focus::FindBar;
            }
            FindMessage::ReplacementChanged(replacement) => {
                self.find.replacement = replacement;
                self.focus = Focus::FindBar;
            }
            FindMessage::ToggleCase(value) => self.find.options.match_case = value,
            FindMessage::ToggleWord(value) => self.find.options.whole_word = value,
            FindMessage::ModeSelected(mode) => self.find.options.mode = mode,
            FindMessage::ToggleDotNewline(value) => self.find.options.dot_matches_newline = value,
            FindMessage::ToggleWrap(value) => self.find.wrap = value,
            FindMessage::ToggleMarkLines(value) => self.find.mark_lines = value,
            FindMessage::ToggleInSelection(enabled) => {
                let selection = self
                    .workspace
                    .active()
                    .map(|d| d.editor.selection().range());
                self.find.scope = match selection {
                    Some(range) if enabled && !range.is_empty() => Some(range),
                    _ => {
                        if enabled {
                            self.find.status = Some("Selecciona primero el texto".to_owned());
                        }
                        None
                    }
                };
            }
            FindMessage::Submit => {
                let forward = !self.modifiers.shift();
                self.find_step(forward);
            }
            FindMessage::Next => self.find_step(true),
            FindMessage::Previous => self.find_step(false),
            FindMessage::Count => {
                if let Some(matches) = self.find_matches() {
                    let n = matches.len();
                    self.find.status = Some(match n {
                        1 => "1 coincidencia".to_owned(),
                        n => format!("{n} coincidencias"),
                    });
                }
            }
            FindMessage::FindAll { all_documents } => self.find_all(all_documents),
            FindMessage::Replace => self.replace_one(),
            FindMessage::ReplaceAll => self.replace_all(),
            FindMessage::MarkAll => self.mark_all(),
            FindMessage::ClearMarks => {
                if let Some(id) = self.workspace.active_id() {
                    self.marks.remove(&id);
                    self.mark_cache.remove(&id);
                }
            }
            FindMessage::CopyBookmarkedLines => {
                let Some(document) = self.workspace.active() else {
                    return Task::none();
                };
                let lines = document.editor.bookmarks().len();
                let text = document.editor.bookmarked_lines_text();
                self.find.status = Some(format!("{lines} líneas copiadas"));
                return iced::clipboard::write(text);
            }
            FindMessage::DeleteBookmarkedLines => {
                if let Some(document) = self.workspace.active_mut() {
                    match document.editor.delete_bookmarked_lines() {
                        Ok(n) => self.find.status = Some(format!("{n} líneas borradas")),
                        Err(error) => self.find.status = Some(error.to_string()),
                    }
                }
            }
            FindMessage::ClearBookmarks => {
                if let Some(document) = self.workspace.active_mut() {
                    document.editor.clear_bookmarks();
                }
            }
            FindMessage::Close => self.escape(),
            FindMessage::OpenHit { group, hit } => self.open_hit(group, hit),
            FindMessage::CloseResults => self.results = None,
            FindMessage::GotoChanged(input) => {
                self.goto = Some(input);
                self.focus = Focus::GoTo;
            }
            FindMessage::GotoSubmit => self.submit_goto(),
        }
        Task::none()
    }

    fn replace_one(&mut self) {
        let Some(query) = self.find_query() else {
            return;
        };
        let Some(matches) = self.find_matches() else {
            return;
        };
        let replacement = self.find.replacement.clone();
        if let Some(document) = self.workspace.active_mut() {
            let selection = document.editor.selection().range();
            if match_index(&matches, &selection).is_some()
                && let Some(text) =
                    query.replacement(&document.editor.text(), selection, &replacement)
                && let Err(error) = document.editor.insert_text(&text)
            {
                self.find.status = Some(error.to_string());
                return;
            }
        }
        // Tras sustituir (o si la selección no era una coincidencia), ir a la siguiente.
        self.find_step(true);
    }

    fn replace_all(&mut self) {
        let Some(query) = self.find_query() else {
            return;
        };
        let replacement = self.find.replacement.clone();
        let scope = self.find.scope.take();
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let replacements = query.replace_all(&document.editor.text(), &replacement, scope);
        self.find.status = Some(match document.editor.replace_ranges(&replacements) {
            Ok(1) => "1 sustitución".to_owned(),
            Ok(n) => format!("{n} sustituciones"),
            Err(error) => error.to_string(),
        });
        self.reveal_active_cursor();
    }

    fn mark_all(&mut self) {
        let Some(query) = self.find_query() else {
            return;
        };
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let matches = query.find_all(&document.editor.text(), MAX_MATCHES);
        if self.find.mark_lines {
            document
                .editor
                .bookmark_lines_at(matches.iter().map(|m| m.start));
        }
        self.find.status = Some(format!("{} marcadas", matches.len()));
        let id = document.id();
        self.marks.insert(id, query);
    }

    fn find_all(&mut self, all_documents: bool) {
        let Some(query) = self.find_query() else {
            return;
        };
        let ids: Vec<DocumentId> = if all_documents {
            self.workspace.tabs().to_vec()
        } else {
            self.workspace.active_id().into_iter().collect()
        };
        let scope = if all_documents {
            None
        } else {
            self.find.scope.clone()
        };
        let mut groups = Vec::new();
        let mut total = 0;
        for id in ids {
            let Some(document) = self.workspace.document(id) else {
                continue;
            };
            let mut matches = query.find_all(&document.editor.text(), MAX_MATCHES);
            if let Some(scope) = &scope {
                matches.retain(|m| m.start >= scope.start && m.end <= scope.end);
            }
            if matches.is_empty() {
                continue;
            }
            total += matches.len();
            groups.push(ResultGroup {
                id,
                name: document.display_name(),
                revision: document.editor.revision(),
                hits: build_hits(document.editor.buffer(), &matches),
            });
        }
        self.find.status = Some(format!(
            "{total} coincidencias en {} documentos",
            groups.len()
        ));
        self.results = Some(SearchResults {
            pattern: self.find.pattern.clone(),
            groups,
            total,
        });
    }

    fn open_hit(&mut self, group: usize, hit: usize) {
        let Some(results) = &self.results else { return };
        let Some(group) = results.groups.get(group) else {
            return;
        };
        let Some(hit) = group.hits.get(hit).cloned() else {
            return;
        };
        let revision = group.revision;
        if !self.workspace.activate(group.id) {
            self.find.status = Some("El documento ya no está abierto".to_owned());
            return;
        }
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        if document.editor.revision() == revision {
            document
                .editor
                .set_selection(Selection::new(hit.range.start, hit.range.end));
        } else {
            // El documento cambió desde la búsqueda: al menos se va a la línea.
            let start = document.editor.buffer().line_start(hit.line);
            document.editor.set_cursor(start, false);
        }
        self.focus = Focus::Editor;
        self.reveal_active_cursor();
    }

    fn submit_goto(&mut self) {
        let Some(input) = &self.goto else { return };
        let Some((line, column)) = parse_goto(input) else {
            self.find.status = Some("Escribe un número de línea".to_owned());
            return;
        };
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let index = document
            .editor
            .buffer()
            .position_to_char(Position::new(line, column));
        document.editor.set_cursor(index, false);
        self.goto = None;
        self.focus = Focus::Editor;
        self.reveal_active_cursor();
    }
}

/// Texto seleccionado o, si no hay selección, la palabra bajo el cursor (o justo antes,
/// si el cursor está tras ella). Si no hay palabra, no toca la selección.
pub fn select_word_near_cursor(editor: &mut lorca_core::EditorState) -> Option<String> {
    if editor.selection().is_empty() {
        let head = editor.selection().head;
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let buffer = editor.buffer();
        let index = if buffer.char_at(head).is_some_and(is_word) {
            head
        } else if head > 0 && buffer.char_at(head - 1).is_some_and(is_word) {
            head - 1
        } else {
            return None;
        };
        editor.select_word_at(index);
    }
    editor
        .selected_text()
        .filter(|text| !text.trim().is_empty())
}

/// Órdenes de marcadores de línea desde atajos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookmarkAction {
    Toggle,
    Next,
    Previous,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_core::EditorState;

    #[test]
    fn word_near_cursor() {
        let mut editor = EditorState::from_text("bodas de sangre");
        editor.set_cursor(2, false);
        assert_eq!(
            select_word_near_cursor(&mut editor).as_deref(),
            Some("bodas")
        );
        // Justo después de una palabra: se toma esa palabra.
        editor.set_cursor(8, false);
        assert_eq!(select_word_near_cursor(&mut editor).as_deref(), Some("de"));
    }

    #[test]
    fn no_word_keeps_cursor() {
        let mut editor = EditorState::from_text("a  b");
        editor.set_cursor(2, false);
        assert_eq!(select_word_near_cursor(&mut editor), None);
        assert!(editor.selection().is_empty());
    }
}
