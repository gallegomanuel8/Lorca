//! Modelo de documentos abiertos y pestañas, independiente de la interfaz.
//!
//! El [`Workspace`] guarda los documentos, el orden de las pestañas y la pestaña activa.
//! La vista solo lo consulta y le envía órdenes.

mod document;
mod error;
mod workspace;

pub use document::{Document, DocumentId};
pub use error::WorkspaceError;
pub use workspace::Workspace;
