//! Área de edición: widget propio que dibuja directamente desde [`lorca_core::EditorState`].
//!
//! El widget no modifica el documento: traduce eventos en [`EditorAction`] que la
//! aplicación aplica sobre el núcleo con [`apply_action`].

mod action;
mod keymap;
mod palette;
mod view;
mod widget;

pub use action::{ClickKind, EditorAction, apply_action};
pub use view::{DocView, Viewport};
pub use widget::{Decorations, ViewOptions, code_editor};

/// Tamaño de la fuente del editor, en píxeles lógicos.
pub const FONT_SIZE: f32 = 14.0;
/// Altura de cada línea, en píxeles lógicos.
pub const LINE_HEIGHT: f32 = 20.0;
/// Anchura de un tabulador, en celdas.
pub const TAB_WIDTH: usize = 4;
/// Texto que inserta la tecla Tab.
pub const INDENT: &str = "    ";
/// Margen entre el canal de números de línea y el texto.
pub const TEXT_PADDING: f32 = 6.0;
