//! Resaltado de sintaxis con tree-sitter.
//!
//! El resultado se expresa como tramos por línea con columnas de carácter, listos para que
//! la vista los pinte sin conocer tree-sitter.

use std::sync::OnceLock;

use tree_sitter_highlight::{Highlight, HighlightConfiguration, HighlightEvent, Highlighter};

use crate::Language;

/// Tamaño máximo de texto que se resalta; por encima se muestra sin colores.
pub const MAX_HIGHLIGHT_BYTES: usize = 4 * 1024 * 1024;

/// Categoría visual de un fragmento de texto. El tema asigna un color a cada una.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Keyword,
    Function,
    Type,
    String,
    Escape,
    Number,
    Constant,
    Comment,
    Property,
    Variable,
    Operator,
    Punctuation,
    Tag,
    Attribute,
    Heading,
    Emphasis,
    Strong,
    Link,
    Code,
}

/// Nombres de captura reconocidos y su categoría. tree-sitter-highlight asigna a cada
/// captura el nombre reconocido más largo que sea prefijo suyo (`function.method` →
/// `function`). Las capturas no listadas (p. ej. `variable`) quedan sin color.
const CAPTURES: &[(&str, HighlightKind)] = &[
    ("keyword", HighlightKind::Keyword),
    ("conditional", HighlightKind::Keyword),
    ("repeat", HighlightKind::Keyword),
    ("storageclass", HighlightKind::Keyword),
    ("include", HighlightKind::Keyword),
    ("import", HighlightKind::Keyword),
    ("media", HighlightKind::Keyword),
    ("charset", HighlightKind::Keyword),
    ("keyframes", HighlightKind::Keyword),
    ("supports", HighlightKind::Keyword),
    ("function", HighlightKind::Function),
    ("type", HighlightKind::Type),
    ("constructor", HighlightKind::Type),
    ("namespace", HighlightKind::Type),
    ("string", HighlightKind::String),
    ("string.special.key", HighlightKind::Property),
    ("string.escape", HighlightKind::Escape),
    ("escape", HighlightKind::Escape),
    ("number", HighlightKind::Number),
    ("float", HighlightKind::Number),
    ("boolean", HighlightKind::Constant),
    ("constant", HighlightKind::Constant),
    ("comment", HighlightKind::Comment),
    ("property", HighlightKind::Property),
    ("field", HighlightKind::Property),
    ("variable.builtin", HighlightKind::Variable),
    ("variable.parameter", HighlightKind::Variable),
    ("parameter", HighlightKind::Variable),
    ("operator", HighlightKind::Operator),
    ("punctuation", HighlightKind::Punctuation),
    ("tag", HighlightKind::Tag),
    ("attribute", HighlightKind::Attribute),
    ("label", HighlightKind::Attribute),
    ("text.title", HighlightKind::Heading),
    ("text.emphasis", HighlightKind::Emphasis),
    ("text.strong", HighlightKind::Strong),
    ("text.uri", HighlightKind::Link),
    ("text.reference", HighlightKind::Link),
    ("text.literal", HighlightKind::Code),
];

/// Tramo resaltado dentro de una línea, en columnas de carácter `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledSpan {
    pub start: usize,
    pub end: usize,
    pub kind: HighlightKind,
}

/// Resaltado de un documento: tramos ordenados y sin solapes para cada línea.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Highlights {
    lines: Vec<Vec<StyledSpan>>,
}

impl Highlights {
    /// Tramos de la línea indicada (vacío si no existe).
    pub fn line(&self, line: usize) -> &[StyledSpan] {
        self.lines.get(line).map_or(&[], Vec::as_slice)
    }

    pub fn len_lines(&self) -> usize {
        self.lines.len()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SyntaxError {
    #[error("el texto es demasiado grande para resaltarlo ({0} bytes)")]
    TooLarge(usize),

    #[error("no se pudieron cargar las reglas de resaltado de {language}: {message}")]
    Query {
        language: &'static str,
        message: String,
    },

    #[error("error al resaltar: {0}")]
    Highlight(#[from] tree_sitter_highlight::Error),
}

/// Resalta `text` (con saltos LF). Devuelve `None` para texto plano.
pub fn highlight(language: Language, text: &str) -> Result<Option<Highlights>, SyntaxError> {
    let Some(grammar) = Grammar::of(language) else {
        return Ok(None);
    };
    if text.len() > MAX_HIGHLIGHT_BYTES {
        return Err(SyntaxError::TooLarge(text.len()));
    }
    let config = grammar.config()?;

    let mut highlighter = Highlighter::new();
    let events = highlighter.highlight(config, text.as_bytes(), None, None, |name| {
        Grammar::from_injection(name).and_then(|g| g.config().ok())
    })?;

    let mut lines: Vec<Vec<StyledSpan>> = vec![Vec::new()];
    let mut stack: Vec<Option<HighlightKind>> = Vec::new();
    let mut column = 0;
    for event in events {
        match event? {
            HighlightEvent::HighlightStart(Highlight(index)) => {
                stack.push(CAPTURES.get(index).map(|(_, kind)| *kind));
            }
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let kind = stack.iter().rev().flatten().next().copied();
                for c in text.get(start..end).unwrap_or_default().chars() {
                    if c == '\n' {
                        lines.push(Vec::new());
                        column = 0;
                        continue;
                    }
                    if let (Some(kind), Some(spans)) = (kind, lines.last_mut()) {
                        match spans.last_mut() {
                            Some(last) if last.end == column && last.kind == kind => last.end += 1,
                            _ => spans.push(StyledSpan {
                                start: column,
                                end: column + 1,
                                kind,
                            }),
                        }
                    }
                    column += 1;
                }
            }
        }
    }
    Ok(Some(Highlights { lines }))
}

/// Gramáticas disponibles (incluida la de Markdown en línea, que solo se usa inyectada).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grammar {
    Markdown,
    MarkdownInline,
    Sql,
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Json,
    Yaml,
    Toml,
    Html,
    Css,
    Bash,
}

const GRAMMAR_COUNT: usize = 13;

/// Configuraciones compiladas bajo demanda (compilar las consultas cuesta milisegundos).
static CONFIGS: [OnceLock<Result<HighlightConfiguration, String>>; GRAMMAR_COUNT] =
    [const { OnceLock::new() }; GRAMMAR_COUNT];

impl Grammar {
    fn of(language: Language) -> Option<Self> {
        Some(match language {
            Language::PlainText => return None,
            Language::Markdown => Self::Markdown,
            Language::Sql => Self::Sql,
            Language::Rust => Self::Rust,
            Language::Python => Self::Python,
            Language::JavaScript => Self::JavaScript,
            Language::TypeScript => Self::TypeScript,
            Language::Json => Self::Json,
            Language::Yaml => Self::Yaml,
            Language::Toml => Self::Toml,
            Language::Html => Self::Html,
            Language::Css => Self::Css,
            Language::Bash => Self::Bash,
        })
    }

    /// Gramática para un nombre de inyección (`markdown_inline`, `javascript`, `rs`…).
    fn from_injection(name: &str) -> Option<Self> {
        if name == "markdown_inline" {
            return Some(Self::MarkdownInline);
        }
        Language::from_name(name).and_then(Self::of)
    }

    fn index(self) -> usize {
        self as usize
    }

    fn config(self) -> Result<&'static HighlightConfiguration, SyntaxError> {
        let slot = CONFIGS
            .get(self.index())
            .ok_or_else(|| SyntaxError::Query {
                language: self.name(),
                message: "gramática desconocida".to_owned(),
            })?;
        slot.get_or_init(|| self.build())
            .as_ref()
            .map_err(|message| SyntaxError::Query {
                language: self.name(),
                message: message.clone(),
            })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::MarkdownInline => "markdown_inline",
            Self::Sql => "sql",
            Self::Rust => "rust",
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Json => "json",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
            Self::Html => "html",
            Self::Css => "css",
            Self::Bash => "bash",
        }
    }

    fn build(self) -> Result<HighlightConfiguration, String> {
        use tree_sitter_javascript as js;
        use tree_sitter_typescript as ts;

        let (language, highlights, injections, locals) = match self {
            Self::Markdown => (
                tree_sitter_md::LANGUAGE,
                tree_sitter_md::HIGHLIGHT_QUERY_BLOCK.to_owned(),
                markdown_block_injections(),
                String::new(),
            ),
            Self::MarkdownInline => (
                tree_sitter_md::INLINE_LANGUAGE,
                tree_sitter_md::HIGHLIGHT_QUERY_INLINE.to_owned(),
                tree_sitter_md::INJECTION_QUERY_INLINE,
                String::new(),
            ),
            Self::Sql => (
                tree_sitter_sequel::LANGUAGE,
                tree_sitter_sequel::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::Rust => (
                tree_sitter_rust::LANGUAGE,
                tree_sitter_rust::HIGHLIGHTS_QUERY.to_owned(),
                tree_sitter_rust::INJECTIONS_QUERY,
                String::new(),
            ),
            Self::Python => (
                tree_sitter_python::LANGUAGE,
                tree_sitter_python::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::JavaScript => (
                js::LANGUAGE,
                format!("{}\n{}", js::HIGHLIGHT_QUERY, js::JSX_HIGHLIGHT_QUERY),
                js::INJECTIONS_QUERY,
                js::LOCALS_QUERY.to_owned(),
            ),
            // La gramática TSX acepta TypeScript y JSX. Sus consultas amplían las de
            // JavaScript, como indica el propio paquete de tree-sitter-typescript.
            Self::TypeScript => (
                ts::LANGUAGE_TSX,
                format!(
                    "{}\n{}\n{}",
                    js::HIGHLIGHT_QUERY,
                    js::JSX_HIGHLIGHT_QUERY,
                    ts::HIGHLIGHTS_QUERY
                ),
                js::INJECTIONS_QUERY,
                format!("{}\n{}", js::LOCALS_QUERY, ts::LOCALS_QUERY),
            ),
            Self::Json => (
                tree_sitter_json::LANGUAGE,
                tree_sitter_json::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::Yaml => (
                tree_sitter_yaml::LANGUAGE,
                tree_sitter_yaml::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::Toml => (
                tree_sitter_toml_ng::LANGUAGE,
                tree_sitter_toml_ng::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::Html => (
                tree_sitter_html::LANGUAGE,
                tree_sitter_html::HIGHLIGHTS_QUERY.to_owned(),
                tree_sitter_html::INJECTIONS_QUERY,
                String::new(),
            ),
            Self::Css => (
                tree_sitter_css::LANGUAGE,
                tree_sitter_css::HIGHLIGHTS_QUERY.to_owned(),
                "",
                String::new(),
            ),
            Self::Bash => (
                tree_sitter_bash::LANGUAGE,
                tree_sitter_bash::HIGHLIGHT_QUERY.to_owned(),
                "",
                String::new(),
            ),
        };

        let names: Vec<&str> = CAPTURES.iter().map(|(name, _)| *name).collect();
        let mut config = HighlightConfiguration::new(
            language.into(),
            self.name(),
            &highlights,
            injections,
            &locals,
        )
        .map_err(|error| error.to_string())?;
        config.configure(&names);
        Ok(config)
    }
}

/// Consulta de inyecciones del Markdown de bloque con `injection.include-children` en el
/// texto en línea y en los bloques de código: esos nodos tienen hijos ocultos y, sin esa
/// opción, tree-sitter-highlight los excluiría y la gramática inyectada vería el texto
/// troceado.
fn markdown_block_injections() -> &'static str {
    static QUERY: OnceLock<String> = OnceLock::new();
    QUERY.get_or_init(|| {
        tree_sitter_md::INJECTION_QUERY_BLOCK
            .replace(
                "(#set! injection.language \"markdown_inline\")",
                "(#set! injection.language \"markdown_inline\")\n  (#set! injection.include-children)",
            )
            .replace(
                "(code_fence_content) @injection.content)",
                "(code_fence_content) @injection.content\n  (#set! injection.include-children))",
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Categoría del carácter en (línea, columna), si tiene alguna.
    fn kind_at(h: &Highlights, line: usize, column: usize) -> Option<HighlightKind> {
        h.line(line)
            .iter()
            .find(|s| s.start <= column && column < s.end)
            .map(|s| s.kind)
    }

    fn run(language: Language, text: &str) -> Highlights {
        highlight(language, text).unwrap().unwrap()
    }

    #[test]
    fn every_grammar_compiles() {
        for index in 0..GRAMMAR_COUNT {
            let grammar = [
                Grammar::Markdown,
                Grammar::MarkdownInline,
                Grammar::Sql,
                Grammar::Rust,
                Grammar::Python,
                Grammar::JavaScript,
                Grammar::TypeScript,
                Grammar::Json,
                Grammar::Yaml,
                Grammar::Toml,
                Grammar::Html,
                Grammar::Css,
                Grammar::Bash,
            ][index];
            assert_eq!(grammar.index(), index);
            if let Err(error) = grammar.config() {
                panic!("{}: {error}", grammar.name());
            }
        }
    }

    #[test]
    fn every_language_highlights_something() {
        let samples = [
            (Language::Markdown, "# Título\n"),
            (Language::Sql, "SELECT * FROM poemas;"),
            (Language::Rust, "fn main() {}"),
            (Language::Python, "def f():\n    return 1"),
            (Language::JavaScript, "const x = 1;"),
            (Language::TypeScript, "let x: number = 1;"),
            (Language::Json, "{\"a\": 1}"),
            (Language::Yaml, "clave: valor"),
            (Language::Toml, "[tabla]\nclave = 1"),
            (Language::Html, "<p class=\"x\">hola</p>"),
            (Language::Css, "p { color: red; }"),
            (Language::Bash, "echo \"hola\""),
        ];
        for (language, text) in samples {
            let h = run(language, text);
            let total: usize = (0..h.len_lines()).map(|l| h.line(l).len()).sum();
            assert!(total > 0, "{language:?} no resaltó nada");
        }
    }

    #[test]
    fn plain_text_has_no_highlights() {
        assert_eq!(highlight(Language::PlainText, "hola").unwrap(), None);
    }

    #[test]
    fn rust_categories() {
        let h = run(
            Language::Rust,
            "fn main() {\n    // nota\n    let s = \"ñ\";\n}\n",
        );
        assert_eq!(h.len_lines(), 5);
        assert_eq!(kind_at(&h, 0, 0), Some(HighlightKind::Keyword));
        assert_eq!(kind_at(&h, 0, 3), Some(HighlightKind::Function));
        assert_eq!(kind_at(&h, 1, 6), Some(HighlightKind::Comment));
        assert_eq!(kind_at(&h, 2, 4), Some(HighlightKind::Keyword));
        // Las columnas cuentan caracteres: la ñ ocupa una sola.
        assert_eq!(kind_at(&h, 2, 13), Some(HighlightKind::String));
        assert_eq!(kind_at(&h, 2, 14), Some(HighlightKind::String));
    }

    #[test]
    fn spans_are_sorted_and_disjoint() {
        let h = run(Language::Rust, "pub struct A { b: Vec<u8> } // c\n");
        let spans = h.line(0);
        assert!(spans.windows(2).all(|w| w[0].end <= w[1].start));
        assert!(spans.iter().all(|s| s.start < s.end));
    }

    #[test]
    fn markdown_highlights_inline_and_code_blocks() {
        let text = "# Romance\n\nTexto **fuerte** y `código`.\n\n```rust\nfn f() {}\n```\n";
        let h = run(Language::Markdown, text);
        assert!(h.line(0).iter().any(|s| s.kind == HighlightKind::Heading));
        assert!(h.line(2).iter().any(|s| s.kind == HighlightKind::Strong));
        assert!(h.line(2).iter().any(|s| s.kind == HighlightKind::Code));
        assert_eq!(kind_at(&h, 5, 0), Some(HighlightKind::Keyword));
        assert_eq!(kind_at(&h, 5, 3), Some(HighlightKind::Function));
    }

    #[test]
    fn code_blocks_in_markdown_get_full_highlighting() {
        let h = run(
            Language::Markdown,
            "```rust\nfn main() {\n    // nota\n}\n```\n",
        );
        assert_eq!(kind_at(&h, 1, 3), Some(HighlightKind::Function));
        assert_eq!(kind_at(&h, 2, 6), Some(HighlightKind::Comment));
    }

    #[test]
    fn html_injects_javascript_and_css() {
        let text = "<style>p { color: red; }</style>\n<script>const x = 1;</script>\n";
        let h = run(Language::Html, text);
        assert_eq!(kind_at(&h, 1, 8), Some(HighlightKind::Keyword));
        assert!(h.line(0).iter().any(|s| s.kind == HighlightKind::Property));
    }

    #[test]
    fn sql_keywords() {
        let h = run(Language::Sql, "select nombre from autores where id = 1;");
        assert_eq!(kind_at(&h, 0, 0), Some(HighlightKind::Keyword));
    }

    #[test]
    fn too_large_is_rejected() {
        let text = "a".repeat(MAX_HIGHLIGHT_BYTES + 1);
        assert!(matches!(
            highlight(Language::Rust, &text),
            Err(SyntaxError::TooLarge(_))
        ));
    }
}
