//! Modelo de documentos abiertos, pestañas, sesión y configuración, independiente de la
//! interfaz.
//!
//! El [`Workspace`] guarda los documentos, el orden de las pestañas y la pestaña activa.
//! La vista solo lo consulta y le envía órdenes.

mod config;
mod document;
mod error;
mod session;
mod storage;
mod theme;
mod workspace;

pub use config::{Config, ThemeMode};
pub use document::{Document, DocumentId};
pub use error::WorkspaceError;
pub use session::{RestoreReport, SESSION_VERSION, Session, SessionDocument, SessionStore};
pub use storage::{DiskStamp, data_dir};
pub use theme::{
    Appearance, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, EditorPalette, Rgba, THEMES_DIR, ThemeSet,
    ThemeSource, ThemeSpec, TokenStyle, UiPalette,
};
pub use workspace::{MAX_PANES, Workspace};
