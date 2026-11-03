//! Detección de lenguaje y resaltado de sintaxis con tree-sitter.

mod guess;
mod highlight;
mod language;

pub use highlight::{
    HighlightKind, Highlights, MAX_HIGHLIGHT_BYTES, StyledSpan, SyntaxError, highlight,
};
pub use language::Language;
