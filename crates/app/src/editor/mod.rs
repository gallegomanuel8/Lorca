//! Área de edición: widget propio que dibuja directamente desde [`lorca_core::EditorState`].
//!
//! El widget no modifica el documento: traduce eventos en [`EditorAction`] que la
//! aplicación aplica sobre el núcleo con [`apply_action`].

mod action;
mod keymap;
pub mod palette;
mod rich;
pub mod rich_edit;
mod scrollbar;
mod view;
mod widget;

pub use action::{ClickKind, CompletionKey, EditorAction, apply_action};
pub use palette::{EditorColors, to_color};
pub use rich::{PROSE_FAMILY, rich_editor};
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

/// Desplazamiento de rueda o touchpad que equivale a un paso de zoom, en píxeles.
const PIXELS_PER_ZOOM_STEP: f32 = 40.0;

/// Convierte el desplazamiento de la rueda (con ⌘ o Ctrl pulsado) en pasos de zoom. Los
/// touchpads mandan muchos movimientos pequeños en píxeles: se acumulan en `pending` y se
/// devuelven los pasos enteros. Hacia arriba (o pellizco que en Windows llega como Ctrl +
/// rueda hacia arriba) acerca.
pub fn zoom_steps(pending: &mut f32, delta: iced::mouse::ScrollDelta) -> i8 {
    let lines = match delta {
        iced::mouse::ScrollDelta::Lines { y, .. } => y,
        iced::mouse::ScrollDelta::Pixels { y, .. } => y / PIXELS_PER_ZOOM_STEP,
    };
    *pending += lines;
    let steps = pending.trunc();
    *pending -= steps;
    steps.clamp(-8.0, 8.0) as i8
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::mouse::ScrollDelta;

    #[test]
    fn wheel_lines_are_zoom_steps() {
        let mut pending = 0.0;
        assert_eq!(
            zoom_steps(&mut pending, ScrollDelta::Lines { x: 0.0, y: 1.0 }),
            1
        );
        assert_eq!(
            zoom_steps(&mut pending, ScrollDelta::Lines { x: 0.0, y: -2.0 }),
            -2
        );
    }

    #[test]
    fn touchpad_pixels_accumulate() {
        let mut pending = 0.0;
        let small = ScrollDelta::Pixels { x: 0.0, y: 15.0 };
        assert_eq!(zoom_steps(&mut pending, small), 0);
        assert_eq!(zoom_steps(&mut pending, small), 0);
        assert_eq!(zoom_steps(&mut pending, small), 1);
        // Lo que sobra queda para el siguiente movimiento.
        assert!((pending - 5.0 / PIXELS_PER_ZOOM_STEP).abs() < 1e-4);
    }
}
