//! Núcleo de edición de Lorca: buffer, cursores, selecciones, ediciones y deshacer/rehacer.
//!
//! Este crate no depende de ninguna biblioteca de interfaz gráfica. Todo el texto se
//! guarda en memoria con finales de línea LF; el formato original del archivo
//! (codificación y finales de línea) se conserva en [`TextFormat`] y se restaura al guardar.

mod buffer;
mod edit;
mod editor;
mod encoding;
mod error;
mod history;
mod line_ending;
mod position;
mod selection;
mod text_format;

pub use buffer::Buffer;
pub use edit::Edit;
pub use editor::{EditorState, Motion};
pub use encoding::{Decoded, TextEncoding, decode, encode};
pub use error::CoreError;
pub use history::{EditKind, History, Transaction};
pub use line_ending::LineEnding;
pub use position::Position;
pub use selection::Selection;
pub use text_format::{LoadedText, TextFormat, load_bytes, save_bytes};
