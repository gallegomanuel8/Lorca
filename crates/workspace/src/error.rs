//! Errores del modelo de documentos.

use std::path::PathBuf;

use lorca_core::CoreError;

use crate::DocumentId;

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("no se pudo leer «{}»: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("no se pudo escribir «{}»: {source}", path.display())]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("el documento {0:?} no existe")]
    UnknownDocument(DocumentId),

    #[error("el documento no tiene ruta; usa «Guardar como»")]
    NoPath,
}
