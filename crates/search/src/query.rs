//! Consultas de búsqueda compiladas y su aplicación sobre un texto.

use std::fmt;
use std::ops::Range;

use regex::{Regex, RegexBuilder};

use crate::escape::{regex_template, unescape_extended};

/// Cómo se interpreta el texto buscado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SearchMode {
    /// Texto literal.
    #[default]
    Normal,
    /// Texto literal con secuencias `\n`, `\t`, `\0`, `\xHH`, `\uHHHH`.
    Extended,
    /// Expresión regular (sintaxis del crate `regex`).
    Regex,
}

impl SearchMode {
    pub const ALL: [Self; 3] = [Self::Normal, Self::Extended, Self::Regex];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Normal => "Normal",
            Self::Extended => "Extendido",
            Self::Regex => "Regex",
        }
    }
}

impl fmt::Display for SearchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Opciones de búsqueda.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SearchOptions {
    pub mode: SearchMode,
    pub match_case: bool,
    pub whole_word: bool,
    /// Solo en modo regex: `.` también coincide con saltos de línea.
    pub dot_matches_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SearchError {
    #[error("el texto a buscar está vacío")]
    EmptyPattern,

    #[error("expresión regular no válida: {0}")]
    InvalidRegex(String),
}

/// Búsqueda compilada. Internamente todos los modos se traducen a una expresión regular.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pattern: String,
    options: SearchOptions,
    regex: Regex,
}

impl SearchQuery {
    pub fn new(pattern: &str, options: SearchOptions) -> Result<Self, SearchError> {
        if pattern.is_empty() {
            return Err(SearchError::EmptyPattern);
        }
        // El texto en memoria solo tiene LF, así que CRLF y CR buscan un LF.
        let source = match options.mode {
            SearchMode::Normal => regex::escape(pattern),
            SearchMode::Extended => {
                let literal = unescape_extended(pattern)
                    .replace("\r\n", "\n")
                    .replace('\r', "\n");
                regex::escape(&literal)
            }
            SearchMode::Regex => pattern.replace(r"\r\n", r"\n"),
        };
        let regex = RegexBuilder::new(&source)
            .case_insensitive(!options.match_case)
            .multi_line(true)
            .dot_matches_new_line(options.mode == SearchMode::Regex && options.dot_matches_newline)
            .build()
            .map_err(|error| SearchError::InvalidRegex(error.to_string()))?;
        Ok(Self {
            pattern: pattern.to_owned(),

            options,
            regex,
        })
    }

    pub fn pattern(&self) -> &str {
        &self.pattern
    }

    pub fn options(&self) -> SearchOptions {
        self.options
    }

    /// Todas las coincidencias (no vacías), como rangos de carácter, hasta `limit`.
    pub fn find_all(&self, text: &str, limit: usize) -> Vec<Range<usize>> {
        let mut counter = CharCounter::default();
        let mut matches = Vec::new();
        for found in self.regex.find_iter(text) {
            if matches.len() >= limit {
                break;
            }
            if found.is_empty() || !self.is_whole_word(text, found.range()) {
                continue;
            }
            let start = counter.advance(text, found.start());
            let end = counter.advance(text, found.end());
            matches.push(start..end);
        }
        matches
    }

    /// Texto de sustitución para la coincidencia que ocupa exactamente `range` (en
    /// caracteres). `None` si en esa posición no hay una coincidencia.
    pub fn replacement(
        &self,
        text: &str,
        range: Range<usize>,
        replace_with: &str,
    ) -> Option<String> {
        let start = char_to_byte(text, range.start)?;
        let end = char_to_byte(text, range.end)?;
        let captures = self.regex.captures_at(text, start)?;
        let whole = captures.get(0)?;
        if whole.start() != start || whole.end() != end || !self.is_whole_word(text, start..end) {
            return None;
        }
        Some(self.expand(&captures, replace_with))
    }

    /// Sustituciones para todas las coincidencias, opcionalmente limitadas a `scope` (en
    /// caracteres). Listas para `EditorState::replace_ranges`.
    pub fn replace_all(
        &self,
        text: &str,
        replace_with: &str,
        scope: Option<Range<usize>>,
    ) -> Vec<(Range<usize>, String)> {
        let mut counter = CharCounter::default();
        let mut result = Vec::new();
        for captures in self.regex.captures_iter(text) {
            let Some(whole) = captures.get(0) else {
                continue;
            };
            if whole.is_empty() || !self.is_whole_word(text, whole.range()) {
                continue;
            }
            let start = counter.advance(text, whole.start());
            let end = counter.advance(text, whole.end());
            if let Some(scope) = &scope {
                if end > scope.end {
                    break;
                }
                if start < scope.start {
                    continue;
                }
            }
            result.push((start..end, self.expand(&captures, replace_with)));
        }
        result
    }

    fn expand(&self, captures: &regex::Captures<'_>, replace_with: &str) -> String {
        match self.options.mode {
            SearchMode::Normal => replace_with.to_owned(),
            SearchMode::Extended => unescape_extended(replace_with),
            SearchMode::Regex => {
                let mut out = String::new();
                captures.expand(&regex_template(replace_with), &mut out);
                out
            }
        }
    }

    /// Con «palabra completa», la coincidencia no puede tener letras, dígitos o `_` pegados
    /// a ningún lado.
    fn is_whole_word(&self, text: &str, bytes: Range<usize>) -> bool {
        if !self.options.whole_word {
            return true;
        }
        let before = text.get(..bytes.start).and_then(|t| t.chars().next_back());
        let after = text.get(bytes.end..).and_then(|t| t.chars().next());
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Byte en el que empieza el carácter `index` (o el final del texto).
fn char_to_byte(text: &str, index: usize) -> Option<usize> {
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(text.len()))
        .nth(index)
}

/// Convierte posiciones de byte crecientes en índices de carácter en tiempo lineal.
#[derive(Default)]
struct CharCounter {
    byte: usize,
    chars: usize,
}

impl CharCounter {
    fn advance(&mut self, text: &str, byte: usize) -> usize {
        if let Some(slice) = text.get(self.byte..byte) {
            self.chars += slice.chars().count();
            self.byte = byte;
        }
        self.chars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pattern: &str, options: SearchOptions) -> SearchQuery {
        SearchQuery::new(pattern, options).unwrap()
    }

    fn opts(mode: SearchMode) -> SearchOptions {
        SearchOptions {
            mode,
            ..SearchOptions::default()
        }
    }

    #[test]
    fn normal_mode_is_literal_and_case_insensitive_by_default() {
        let q = query("a.b", opts(SearchMode::Normal));
        assert_eq!(q.find_all("A.B axb a.b", 100), vec![0..3, 8..11]);
    }

    #[test]
    fn match_case() {
        let options = SearchOptions {
            match_case: true,
            ..SearchOptions::default()
        };
        assert_eq!(
            query("Luna", options).find_all("luna Luna", 100),
            vec![5..9]
        );
    }

    #[test]
    fn positions_are_in_chars() {
        let q = query("verde", opts(SearchMode::Normal));
        assert_eq!(q.find_all("ñandú verde", 100), vec![6..11]);
    }

    #[test]
    fn whole_word() {
        let options = SearchOptions {
            whole_word: true,
            ..SearchOptions::default()
        };
        let q = query("mar", options);
        assert_eq!(
            q.find_all("mar marea amar mar_ (mar)", 100),
            vec![0..3, 21..24]
        );
    }

    #[test]
    fn extended_mode_understands_escapes_and_crlf() {
        let q = query(r"a\r\nb", opts(SearchMode::Extended));
        assert_eq!(q.find_all("a\nb", 100), vec![0..3]);
        let q = query(r"\t", opts(SearchMode::Extended));
        assert_eq!(q.find_all("x\ty", 100), vec![1..2]);
    }

    #[test]
    fn regex_mode_is_multiline() {
        let q = query(r"^\w+", opts(SearchMode::Regex));
        assert_eq!(q.find_all("uno dos\ntres", 100), vec![0..3, 8..12]);
    }

    #[test]
    fn dot_matches_newline_option() {
        let mut options = opts(SearchMode::Regex);
        assert!(query("a.b", options).find_all("a\nb", 100).is_empty());
        options.dot_matches_newline = true;
        assert_eq!(query("a.b", options).find_all("a\nb", 100), vec![0..3]);
    }

    #[test]
    fn empty_matches_are_skipped() {
        let q = query("x*", opts(SearchMode::Regex));
        assert_eq!(q.find_all("axxb", 100), vec![1..3]);
    }

    #[test]
    fn limit() {
        let q = query("a", opts(SearchMode::Normal));
        assert_eq!(q.find_all("aaaa", 2).len(), 2);
    }

    #[test]
    fn errors() {
        assert_eq!(
            SearchQuery::new("", SearchOptions::default()).unwrap_err(),
            SearchError::EmptyPattern
        );
        assert!(matches!(
            SearchQuery::new("(", opts(SearchMode::Regex)),
            Err(SearchError::InvalidRegex(_))
        ));
        // En modo normal los paréntesis son literales.
        assert!(SearchQuery::new("(", SearchOptions::default()).is_ok());
    }

    #[test]
    fn regex_replacement_with_groups() {
        let q = query(r"(\w+)@(\w+)", opts(SearchMode::Regex));
        let text = "escribe a lorca@granada";
        assert_eq!(
            q.replacement(text, 10..23, r"\2: $1").as_deref(),
            Some("granada: lorca")
        );
        assert_eq!(q.replacement(text, 0..7, "x"), None);
    }

    #[test]
    fn normal_replacement_is_literal() {
        let q = query("a", opts(SearchMode::Normal));
        assert_eq!(q.replacement("a", 0..1, r"$1\n").as_deref(), Some(r"$1\n"));
    }

    #[test]
    fn extended_replacement_unescapes() {
        let q = query(",", opts(SearchMode::Extended));
        assert_eq!(
            q.replace_all("a,b", r"\n", None),
            vec![(1..2, "\n".to_owned())]
        );
    }

    #[test]
    fn replace_all_in_scope() {
        let q = query("o", opts(SearchMode::Normal));
        let all = q.replace_all("oso oso", "0", None);
        assert_eq!(all.len(), 4);
        let scoped = q.replace_all("oso oso", "0", Some(4..7));
        assert_eq!(scoped, vec![(4..5, "0".to_owned()), (6..7, "0".to_owned())]);
    }

    #[test]
    fn replacement_respects_anchors() {
        // `^` solo coincide al principio de línea, también al sustituir una sola.
        let q = query(r"^b", opts(SearchMode::Regex));
        assert_eq!(q.replacement("ab\nb", 1..2, "X"), None);
        assert_eq!(q.replacement("ab\nb", 3..4, "X").as_deref(), Some("X"));
    }
}
