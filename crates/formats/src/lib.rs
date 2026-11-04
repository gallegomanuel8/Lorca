//! Formateadores (JSON, SQL), conversión de mayúsculas, formato con marcas de Markdown
//! (negrita, cursiva, listas) y Markdown a HTML y PDF.
//!
//! Funciones puras sobre texto, sin dependencias de interfaz.

mod case;
mod json;
mod markdown;
mod markup;
mod pdf;
mod sql;

pub use case::{Case, change_case};
pub use json::{FormatError, format_json, minify_json};
pub use markdown::markdown_to_html;
pub use markup::{Inline, ListKind, MarkupEdit, toggle_inline, toggle_list};
pub use pdf::{FontFiles, PdfError, PdfFonts, markdown_to_pdf};
pub use sql::format_sql;
