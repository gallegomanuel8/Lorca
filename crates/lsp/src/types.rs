//! Tipos que el cliente entrega a la aplicación, ya simplificados a partir de los del
//! protocolo.

use std::path::{Path, PathBuf};

use lsp_types::{
    CompletionItemKind, CompletionTextEdit, GotoDefinitionResponse, HoverContents,
    InsertTextFormat, MarkedString,
};
pub use lsp_types::{Position, Range};

use crate::snippet;

/// Identificador que la aplicación da a cada servidor arrancado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Un problema del documento. El rango va en las unidades del servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: Severity,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range,
    pub new_text: String,
}

/// Una propuesta de autocompletado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    /// Abreviatura del tipo de elemento («fn», «var», «tipo»…), vacía si no se sabe.
    pub kind: &'static str,
    pub filter_text: String,
    pub sort_text: String,
    /// Texto que se inserta (snippets ya expandidos).
    pub text: String,
    /// Dónde dejar el cursor, en caracteres desde el inicio de `text`; `None` = al final.
    pub cursor: Option<usize>,
    /// Rango que sustituye (unidades del servidor). `None`: la palabra antes del cursor.
    pub range: Option<Range>,
    /// Ediciones extra (p. ej. añadir un `use`).
    pub additional_edits: Vec<TextEdit>,
}

/// Una posición en otro (o el mismo) archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub range: Range,
}

/// Lo que el cliente comunica a la aplicación.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// El servidor terminó de iniciarse.
    Ready { server: ServerId },
    Diagnostics {
        server: ServerId,
        path: PathBuf,
        diagnostics: Vec<Diagnostic>,
    },
    Completion {
        server: ServerId,
        request: u64,
        items: Vec<CompletionItem>,
    },
    Hover {
        server: ServerId,
        request: u64,
        /// Texto Markdown, o `None` si no hay información.
        text: Option<String>,
    },
    Definition {
        server: ServerId,
        request: u64,
        locations: Vec<Location>,
    },
    /// Mensaje que el servidor quiere enseñar al usuario.
    Message {
        server: ServerId,
        text: String,
        is_error: bool,
    },
    /// Una petición falló (o el servidor contestó con error).
    Failed {
        server: ServerId,
        request: Option<u64>,
        message: String,
    },
    /// El proceso terminó o cerró su salida.
    Exited { server: ServerId, message: String },
}

pub fn path_to_uri(path: &Path) -> Option<lsp_types::Uri> {
    let url = url::Url::from_file_path(path).ok()?;
    url.as_str().parse().ok()
}

pub fn uri_to_path(uri: &lsp_types::Uri) -> Option<PathBuf> {
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

pub(crate) fn severity(value: Option<lsp_types::DiagnosticSeverity>) -> Severity {
    match value {
        Some(lsp_types::DiagnosticSeverity::WARNING) => Severity::Warning,
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => Severity::Information,
        Some(lsp_types::DiagnosticSeverity::HINT) => Severity::Hint,
        // Sin gravedad, el protocolo deja decidir al cliente: se trata como error.
        _ => Severity::Error,
    }
}

pub(crate) fn diagnostic(d: lsp_types::Diagnostic) -> Diagnostic {
    Diagnostic {
        range: d.range,
        severity: severity(d.severity),
        message: d.message,
        source: d.source,
    }
}

fn kind_label(kind: Option<CompletionItemKind>) -> &'static str {
    let Some(kind) = kind else { return "" };
    match kind {
        CompletionItemKind::FUNCTION
        | CompletionItemKind::METHOD
        | CompletionItemKind::CONSTRUCTOR => "fn",
        CompletionItemKind::VARIABLE => "var",
        CompletionItemKind::FIELD | CompletionItemKind::PROPERTY => "campo",
        CompletionItemKind::CLASS
        | CompletionItemKind::STRUCT
        | CompletionItemKind::INTERFACE
        | CompletionItemKind::ENUM
        | CompletionItemKind::TYPE_PARAMETER => "tipo",
        CompletionItemKind::MODULE => "mód",
        CompletionItemKind::CONSTANT => "const",
        CompletionItemKind::ENUM_MEMBER => "valor",
        CompletionItemKind::KEYWORD => "clave",
        CompletionItemKind::SNIPPET => "snip",
        CompletionItemKind::FILE | CompletionItemKind::FOLDER => "ruta",
        _ => "",
    }
}

pub(crate) fn completion_item(item: lsp_types::CompletionItem) -> CompletionItem {
    let (raw, range) = match item.text_edit {
        Some(CompletionTextEdit::Edit(edit)) => (edit.new_text, Some(edit.range)),
        Some(CompletionTextEdit::InsertAndReplace(edit)) => (edit.new_text, Some(edit.insert)),
        None => (
            item.insert_text
                .clone()
                .unwrap_or_else(|| item.label.clone()),
            None,
        ),
    };
    let (text, cursor) = if item.insert_text_format == Some(InsertTextFormat::SNIPPET) {
        let expanded = snippet::expand(&raw);
        (expanded.text, expanded.cursor)
    } else {
        (raw, None)
    };
    CompletionItem {
        filter_text: item.filter_text.unwrap_or_else(|| item.label.clone()),
        sort_text: item.sort_text.unwrap_or_else(|| item.label.clone()),
        kind: kind_label(item.kind),
        detail: item.detail,
        label: item.label,
        text,
        cursor,
        range,
        additional_edits: item
            .additional_text_edits
            .unwrap_or_default()
            .into_iter()
            .map(|e| TextEdit {
                range: e.range,
                new_text: e.new_text,
            })
            .collect(),
    }
}

fn marked(value: MarkedString) -> String {
    match value {
        MarkedString::String(text) => text,
        MarkedString::LanguageString(code) => {
            format!("```{}\n{}\n```", code.language, code.value)
        }
    }
}

pub(crate) fn hover_text(contents: HoverContents) -> Option<String> {
    let text = match contents {
        HoverContents::Scalar(value) => marked(value),
        HoverContents::Array(values) => values
            .into_iter()
            .map(marked)
            .collect::<Vec<_>>()
            .join("\n\n"),
        HoverContents::Markup(markup) => markup.value,
    };
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn locations(response: GotoDefinitionResponse) -> Vec<Location> {
    let pairs: Vec<(lsp_types::Uri, Range)> = match response {
        GotoDefinitionResponse::Scalar(location) => vec![(location.uri, location.range)],
        GotoDefinitionResponse::Array(list) => list.into_iter().map(|l| (l.uri, l.range)).collect(),
        GotoDefinitionResponse::Link(links) => links
            .into_iter()
            .map(|l| (l.target_uri, l.target_selection_range))
            .collect(),
    };
    pairs
        .into_iter()
        .filter_map(|(uri, range)| {
            Some(Location {
                path: uri_to_path(&uri)?,
                range,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn uris_round_trip_with_spaces_and_accents() {
        let path = if cfg!(windows) {
            PathBuf::from(r"C:\Users\Federico\Poemas de ñ\romance.rs")
        } else {
            PathBuf::from("/Users/federico/Poemas de ñ/romance.rs")
        };
        let uri = path_to_uri(&path).unwrap();
        assert!(uri.as_str().starts_with("file:///"));
        assert!(uri.as_str().contains("Poemas%20de%20%C3%B1"));
        assert_eq!(uri_to_path(&uri), Some(path));
    }

    #[test]
    fn completion_with_snippet_text_edit() {
        let item: lsp_types::CompletionItem = serde_json::from_value(json!({
            "label": "println!",
            "kind": 3,
            "insertTextFormat": 2,
            "textEdit": {
                "range": {"start": {"line": 0, "character": 4}, "end": {"line": 0, "character": 7}},
                "newText": "println!(\"$1\")$0"
            }
        }))
        .unwrap();
        let item = completion_item(item);
        assert_eq!(item.text, "println!(\"\")");
        assert_eq!(item.cursor, Some(10));
        assert_eq!(item.kind, "fn");
        assert_eq!(item.range.unwrap().start.character, 4);
        assert_eq!(item.filter_text, "println!");
    }

    #[test]
    fn completion_without_edit_uses_insert_text_or_label() {
        let item: lsp_types::CompletionItem =
            serde_json::from_value(json!({"label": "verde"})).unwrap();
        let item = completion_item(item);
        assert_eq!(item.text, "verde");
        assert_eq!(item.range, None);
    }

    #[test]
    fn hover_contents_become_markdown() {
        let contents: HoverContents = serde_json::from_value(json!([
            {"language": "rust", "value": "fn luna()"},
            "Devuelve la luna."
        ]))
        .unwrap();
        assert_eq!(
            hover_text(contents).unwrap(),
            "```rust\nfn luna()\n```\n\nDevuelve la luna."
        );
        let empty: HoverContents =
            serde_json::from_value(json!({"kind": "markdown", "value": "  "})).unwrap();
        assert_eq!(hover_text(empty), None);
    }
}
