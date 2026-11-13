//! Modelo de documentos abiertos, pestañas, sesión y configuración, independiente de la
//! interfaz.
//!
//! El [`Workspace`] guarda los documentos, el orden de las pestañas y la pestaña activa.
//! La vista solo lo consulta y le envía órdenes.

mod config;
mod document;
mod error;
mod images;
pub mod instance;
mod session;
mod storage;
mod theme;
mod workspace;

pub use config::{Config, LspConfig, LspServer, SearchConfig, ThemeMode, WELCOME_VERSION};
pub use document::{Document, DocumentId};
pub use error::WorkspaceError;
pub use images::{
    IMAGE_EXTENSIONS, MAX_IMAGE_BYTES, MAX_IMAGE_SIDE, clean_scratch_images, image_alt,
    image_target, images_dir, import_image, import_scratch_image, is_image_path, relocate_images,
    resolve_image, save_png, save_scratch_png, scratch_images_dir,
};
pub use session::{RestoreReport, SESSION_VERSION, Session, SessionDocument, SessionStore};
pub use storage::{DiskStamp, data_dir};
pub use theme::{
    Appearance, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, EditorPalette, Rgba, THEMES_DIR, ThemeSet,
    ThemeSource, ThemeSpec, TokenStyle, UiPalette,
};
pub use workspace::{MAX_PANES, Workspace};
