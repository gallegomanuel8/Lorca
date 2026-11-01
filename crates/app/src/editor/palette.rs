//! Colores y estilos del resaltado de sintaxis para los temas claro y oscuro.
//!
//! Provisional hasta los temas configurables de la Fase 6.

use iced::font::{Style, Weight};
use iced::{Color, Font, color};
use lorca_syntax::HighlightKind;

/// Color y fuente de un tramo. `None` como color significa «el color de texto del tema».
pub fn style(kind: Option<HighlightKind>, is_dark: bool) -> (Option<Color>, Font) {
    let Some(kind) = kind else {
        return (None, Font::MONOSPACE);
    };
    let color = if is_dark { dark(kind) } else { light(kind) };
    let font = match kind {
        HighlightKind::Heading | HighlightKind::Strong => Font {
            weight: Weight::Bold,
            ..Font::MONOSPACE
        },
        HighlightKind::Emphasis => Font {
            style: Style::Italic,
            ..Font::MONOSPACE
        },
        _ => Font::MONOSPACE,
    };
    (color, font)
}

/// Paleta oscura, inspirada en One Dark.
fn dark(kind: HighlightKind) -> Option<Color> {
    Some(match kind {
        HighlightKind::Keyword => color!(0xc678dd),
        HighlightKind::Function | HighlightKind::Link => color!(0x61afef),
        HighlightKind::Type => color!(0xe5c07b),
        HighlightKind::String | HighlightKind::Code => color!(0x98c379),
        HighlightKind::Escape | HighlightKind::Operator => color!(0x56b6c2),
        HighlightKind::Number | HighlightKind::Constant | HighlightKind::Attribute => {
            color!(0xd19a66)
        }
        HighlightKind::Comment => color!(0x7f848e),
        HighlightKind::Property | HighlightKind::Variable | HighlightKind::Tag => {
            color!(0xe06c75)
        }
        HighlightKind::Heading => color!(0xe06c75),
        HighlightKind::Strong => color!(0xd19a66),
        HighlightKind::Emphasis => color!(0xc678dd),
        HighlightKind::Punctuation => return None,
    })
}

/// Paleta clara, inspirada en GitHub Light.
fn light(kind: HighlightKind) -> Option<Color> {
    Some(match kind {
        HighlightKind::Keyword => color!(0xcf222e),
        HighlightKind::Function => color!(0x8250df),
        HighlightKind::Type | HighlightKind::Variable => color!(0x953800),
        HighlightKind::String | HighlightKind::Code | HighlightKind::Link => color!(0x0a3069),
        HighlightKind::Escape
        | HighlightKind::Number
        | HighlightKind::Constant
        | HighlightKind::Property
        | HighlightKind::Attribute
        | HighlightKind::Heading => color!(0x0550ae),
        HighlightKind::Comment => color!(0x6e7781),
        HighlightKind::Tag => color!(0x116329),
        HighlightKind::Operator
        | HighlightKind::Punctuation
        | HighlightKind::Strong
        | HighlightKind::Emphasis => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_uses_theme_color() {
        assert_eq!(style(None, true), (None, Font::MONOSPACE));
    }

    #[test]
    fn keywords_are_colored_in_both_themes() {
        assert!(style(Some(HighlightKind::Keyword), true).0.is_some());
        assert!(style(Some(HighlightKind::Keyword), false).0.is_some());
    }

    #[test]
    fn strong_is_bold() {
        assert_eq!(
            style(Some(HighlightKind::Strong), false).1.weight,
            Weight::Bold
        );
    }
}
