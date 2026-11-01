//! Formateadores (JSON, SQL), conversión de mayúsculas y Markdown a HTML y PDF.
//!
//! Funciones puras sobre texto, sin dependencias de interfaz.

mod case;
mod json;
mod markdown;
mod pdf;
mod sql;

pub use case::{Case, change_case};
pub use json::{FormatError, format_json, minify_json};
pub use markdown::markdown_to_html;
pub use pdf::{FontFiles, PdfError, PdfFonts, markdown_to_pdf};
pub use sql::format_sql;
