//! Cliente del Language Server Protocol, independiente de la interfaz.
//!
//! - [`Client`]: lanza un servidor (o usa un canal ya abierto), lo inicializa y traduce
//!   sus respuestas y notificaciones en [`Event`].
//! - [`servers`]: servidores conocidos por lenguaje, búsqueda en el `PATH` y raíz del
//!   proyecto.
//! - [`PositionEncoding`]: conversión de columnas entre el editor (caracteres) y el
//!   servidor.
//!
//! El documento se sincroniza siempre completo (un `didChange` con todo el texto).

mod client;
mod error;
mod framing;
mod position;
pub mod servers;
mod snippet;
mod types;

pub use client::{Capabilities, Client};
pub use error::LspError;
pub use position::PositionEncoding;
pub use servers::{ServerCommand, find_root, known_servers, language_id};
pub use snippet::{Expanded, expand as expand_snippet};
pub use types::{
    CompletionItem, Diagnostic, Event, Location, Position, Range, ServerId, Severity, TextEdit,
    path_to_uri, uri_to_path,
};
