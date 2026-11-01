//! Búsqueda y sustitución sobre texto, sin dependencias de interfaz.
//!
//! Tres modos, como en Notepad++: **Normal** (literal), **Extendido** (literal con
//! secuencias `\n`, `\t`, `\xHH`…) y **Regex**. Todas las posiciones que se devuelven
//! son índices de **carácter**, igual que en `lorca-core`.

mod escape;
mod navigate;
mod query;

pub use escape::unescape_extended;
pub use navigate::{match_index, next_match, previous_match};
pub use query::{SearchError, SearchMode, SearchOptions, SearchQuery};
