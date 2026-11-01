//! Errores del cliente LSP.

#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("no se pudo iniciar «{command}»: {source}")]
    Spawn {
        command: String,
        source: std::io::Error,
    },

    #[error("error de E/S con el servidor: {0}")]
    Io(#[from] std::io::Error),

    #[error("mensaje mal formado del servidor: {0}")]
    Protocol(String),

    #[error("JSON inválido del servidor: {0}")]
    Json(#[from] serde_json::Error),
}
