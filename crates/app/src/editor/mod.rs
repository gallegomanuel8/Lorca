//! Área de edición: widget propio que dibuja directamente desde [`lorca_core::EditorState`].
//!
//! El widget no modifica el documento: traduce eventos en [`EditorAction`] que la
//! aplicación aplica sobre el núcleo con [`apply_action`].

mod action;
mod keymap;
mod palette;
mod view;
mod widget;

pub use action::{ClickKind, CompletionKey, EditorAction, apply_action};
pub use palette::{EditorColors, to_color};
pub use view::{DocView, Viewport};
pub use widget::{Decorations, Underline, UnderlineKind, ViewOptions, code_editor};

/// Medidas del texto del editor, derivadas de la configuración.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// Tamaño de la fuente, en píxeles lógicos.
    pub font_size: f32,
    /// Altura de cada línea, en píxeles lógicos.
    pub line_height: f32,
    /// Anchura de un tabulador, en celdas.
    pub tab_width: usize,
}

impl Metrics {
    pub fn new(font_size: f32, tab_width: usize) -> Self {
        Self {
            font_size,
            line_height: (font_size * 1.43).round(),
            tab_width,
        }
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new(14.0, 4)
    }
}

/// Texto que inserta la tecla Tab si la aplicación no indica otro.
pub const INDENT: &str = "    ";
/// Margen entre el canal de números de línea y el texto.
pub const TEXT_PADDING: f32 = 6.0;
