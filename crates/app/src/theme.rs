//! Tema activo: el [`ThemeSpec`] elegido convertido a un tema de iced y a los colores del
//! editor.

use iced::Theme;
use iced::theme::{Mode, Palette};
use lorca_workspace::{Appearance, ThemeSet, ThemeSpec};

use crate::editor::{EditorColors, to_color};

/// Tema en uso y la elección que lo produjo (para reconstruirlo solo si cambia).
#[derive(Debug, Clone)]
pub struct ActiveTheme {
    pub appearance: Appearance,
    /// Nombre pedido en la configuración (puede no existir y haberse sustituido).
    pub requested: String,
    pub iced: Theme,
    pub editor: EditorColors,
}

impl ActiveTheme {
    pub fn new(themes: &ThemeSet, name: &str, appearance: Appearance) -> Self {
        let spec = themes.resolve(name, appearance);
        let iced = spec.map_or_else(
            || match appearance {
                Appearance::Light => Theme::Light,
                Appearance::Dark => Theme::Dark,
            },
            iced_theme,
        );
        let editor = EditorColors::new(spec, &iced);
        Self {
            appearance,
            requested: name.to_owned(),
            iced,
            editor,
        }
    }

    /// Nombre del tema que se está mostrando.
    pub fn name(&self) -> String {
        self.iced.to_string()
    }

    pub fn is(&self, name: &str, appearance: Appearance) -> bool {
        self.appearance == appearance && self.requested == name
    }
}

/// Tema de iced con la paleta de la interfaz de `spec`.
pub fn iced_theme(spec: &ThemeSpec) -> Theme {
    let palette = spec.palette;
    Theme::custom(
        spec.name.clone(),
        Palette {
            background: to_color(palette.background),
            text: to_color(palette.text),
            primary: to_color(palette.primary),
            success: to_color(palette.success),
            warning: to_color(palette.warning),
            danger: to_color(palette.danger),
        },
    )
}

/// `Some(true)` si el sistema está en modo oscuro; `None` si no lo indica.
pub fn system_is_dark(mode: Mode) -> Option<bool> {
    match mode {
        Mode::Dark => Some(true),
        Mode::Light => Some(false),
        Mode::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_theme_uses_spec_palette() {
        let set = ThemeSet::builtin();
        let active = ActiveTheme::new(&set, "Yerma", Appearance::Dark);
        assert_eq!(active.name(), "Yerma");
        assert!(active.iced.extended_palette().is_dark);
        assert!(active.is("Yerma", Appearance::Dark));
    }

    #[test]
    fn wrong_appearance_falls_back() {
        let set = ThemeSet::builtin();
        let active = ActiveTheme::new(&set, "Yerma", Appearance::Light);
        assert!(!active.iced.extended_palette().is_dark);
        assert_ne!(active.name(), "Yerma");
    }
}
