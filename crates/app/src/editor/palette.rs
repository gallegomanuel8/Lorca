//! Colores del área de edición y del resaltado, a partir del tema activo.

use iced::font::{Style, Weight};
use iced::{Color, Font, Theme};
use lorca_syntax::HighlightKind;
use lorca_workspace::{Rgba, ThemeSpec, TokenStyle};

/// Color y fuente de una categoría de resaltado. `None` como color significa «el color de
/// texto del tema».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SyntaxStyle {
    pub color: Option<Color>,
    pub font: Font,
}

/// Colores del editor ya resueltos (los que el tema no da se derivan de su paleta).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorColors {
    pub background: Color,
    pub gutter: Color,
    pub text: Color,
    pub line_number: Color,
    pub line_number_active: Color,
    pub current_line: Color,
    pub selection: Color,
    pub cursor: Color,
    pub scrollbar: Color,
    pub found: Color,
    pub found_current: Color,
    pub mark: Color,
    pub bookmark: Color,
    pub whitespace: Color,
    pub guide: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    syntax: [SyntaxStyle; HighlightKind::ALL.len()],
}

pub fn to_color(rgba: Rgba) -> Color {
    Color::from_rgba8(rgba.r, rgba.g, rgba.b, f32::from(rgba.a) / 255.0)
}

fn with_alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

impl EditorColors {
    /// Colores para `theme` (el tema de iced ya construido) con los ajustes de `spec`.
    pub fn new(spec: Option<&ThemeSpec>, theme: &Theme) -> Self {
        let palette = theme.extended_palette();
        let background = palette.background.base;
        let editor = spec.map(|s| s.editor).unwrap_or_default();
        let pick = |value: Option<Rgba>, fallback: Color| value.map_or(fallback, to_color);
        let syntax = HighlightKind::ALL
            .map(|kind| syntax_style(kind, spec.and_then(|s| s.token(kind)).unwrap_or_default()));
        Self {
            background: pick(editor.background, background.color),
            gutter: pick(editor.gutter, palette.background.weakest.color),
            text: background.text,
            line_number: pick(editor.line_number, palette.background.strong.color),
            line_number_active: pick(editor.line_number_active, background.text),
            current_line: pick(editor.current_line, palette.background.weakest.color),
            selection: pick(
                editor.selection,
                with_alpha(palette.primary.base.color, 0.35),
            ),
            cursor: pick(editor.cursor, palette.primary.strong.color),
            found: pick(editor.find, with_alpha(palette.warning.base.color, 0.35)),
            found_current: pick(
                editor.find_current,
                with_alpha(palette.warning.strong.color, 0.75),
            ),
            mark: pick(editor.mark, with_alpha(palette.success.base.color, 0.35)),
            bookmark: pick(editor.bookmark, palette.primary.base.color),
            whitespace: pick(
                editor.whitespace,
                with_alpha(palette.background.strong.color, 0.7),
            ),
            guide: pick(
                editor.guide,
                with_alpha(palette.background.strong.color, 0.35),
            ),
            scrollbar: with_alpha(palette.background.strong.color, 0.5),
            error: pick(editor.error, palette.danger.base.color),
            warning: pick(editor.warning, palette.warning.base.color),
            info: pick(editor.info, palette.primary.base.color),
            syntax,
        }
    }

    /// Color y fuente de un tramo.
    pub fn style(&self, kind: Option<HighlightKind>) -> SyntaxStyle {
        kind.and_then(|kind| self.syntax.get(kind as usize).copied())
            .unwrap_or(SyntaxStyle {
                color: None,
                font: Font::MONOSPACE,
            })
    }
}

fn syntax_style(kind: HighlightKind, token: TokenStyle) -> SyntaxStyle {
    // Sin indicación del tema, los títulos y la negrita de Markdown van en negrita y el
    // énfasis en cursiva.
    let bold = token.bold || matches!(kind, HighlightKind::Heading | HighlightKind::Strong);
    let italic = token.italic || kind == HighlightKind::Emphasis;
    SyntaxStyle {
        color: token.color.map(to_color),
        font: Font {
            weight: if bold { Weight::Bold } else { Weight::Normal },
            style: if italic { Style::Italic } else { Style::Normal },
            ..Font::MONOSPACE
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_workspace::ThemeSet;

    fn colors(name: &str) -> EditorColors {
        let set = ThemeSet::builtin();
        let spec = set.get(name).unwrap();
        EditorColors::new(Some(spec), &Theme::Dark)
    }

    #[test]
    fn plain_text_uses_theme_color() {
        let style = colors("Yerma").style(None);
        assert_eq!(style.color, None);
        assert_eq!(style.font, Font::MONOSPACE);
    }

    #[test]
    fn theme_colors_reach_syntax_and_editor() {
        let yerma = colors("Yerma");
        assert_eq!(
            yerma.style(Some(HighlightKind::Keyword)).color,
            Some(Color::from_rgb8(0xd0, 0x8b, 0x4c))
        );
        assert_eq!(yerma.cursor, Color::from_rgb8(0xf0, 0xc2, 0x7a));
        let comment = yerma.style(Some(HighlightKind::Comment));
        assert_eq!(comment.font.style, Style::Italic);
    }

    #[test]
    fn strong_is_bold_even_without_theme() {
        let plain = EditorColors::new(None, &Theme::Light);
        assert_eq!(
            plain.style(Some(HighlightKind::Strong)).font.weight,
            Weight::Bold
        );
        assert_eq!(plain.style(Some(HighlightKind::Keyword)).color, None);
    }
}
