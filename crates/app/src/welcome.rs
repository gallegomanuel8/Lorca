//! Pantalla de bienvenida del primer arranque: elección del tema.

use iced::theme::Mode;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme};
use lorca_workspace::{Appearance, Config, ThemeMode, ThemeSet};

use crate::app::Message;
use crate::theme::{ActiveTheme, system_is_dark};

/// Líneas de ejemplo que se muestran en las vistas previas.
const SAMPLE: [&str; 4] = [
    "fn romance() {",
    "    let verde = \"viento\";",
    "    sueña(verde);",
    "}",
];

const PREVIEW_WIDTH: f32 = 190.0;
const PREVIEW_HEIGHT: f32 = 92.0;

/// Vista de bienvenida con la opción `selected` marcada.
pub fn view(
    selected: ThemeMode,
    system: Mode,
    themes: &ThemeSet,
    config: &Config,
) -> Element<'static, Message> {
    let theme_for =
        |appearance| ActiveTheme::new(themes, config.theme_name(appearance), appearance);
    let light = theme_for(Appearance::Light);
    let dark = theme_for(Appearance::Dark);
    let cards = row(ThemeMode::ALL.map(|mode| {
        card(
            mode,
            mode == selected,
            system_is_dark(system),
            &light,
            &dark,
        )
    }))
    .spacing(16);

    let content = column![
        text("Bienvenido a Lorca").size(30),
        text("Verde que te quiero verde.").size(14).font(Font {
            style: iced::font::Style::Italic,
            ..Font::default()
        }),
        Space::new().height(12),
        text(
            "¿Cómo quieres ver el editor? Podrás cambiarlo cuando quieras desde la barra de estado."
        )
        .size(14),
        Space::new().height(8),
        cards,
        Space::new().height(12),
        button(text("Empezar").size(14))
            .padding([8, 24])
            .style(button::primary)
            .on_press(Message::WelcomeDone),
    ]
    .spacing(8)
    .align_x(Alignment::Center);

    container(content).center(Length::Fill).into()
}

/// Tarjeta seleccionable con la vista previa de un modo.
fn card(
    mode: ThemeMode,
    is_selected: bool,
    system_dark: Option<bool>,
    light: &ActiveTheme,
    dark: &ActiveTheme,
) -> Element<'static, Message> {
    let preview: Element<'static, Message> = match mode {
        // En automático se muestran ambos, mitad y mitad.
        ThemeMode::Auto => row![
            preview(light, PREVIEW_WIDTH / 2.0),
            preview(dark, PREVIEW_WIDTH / 2.0)
        ]
        .into(),
        _ => match mode.appearance(system_dark) {
            Appearance::Light => preview(light, PREVIEW_WIDTH),
            Appearance::Dark => preview(dark, PREVIEW_WIDTH),
        },
    };
    let label = column![
        text(mode.label()).size(14),
        text(match mode {
            ThemeMode::Auto => "Sigue al sistema".to_owned(),
            ThemeMode::Light => light.name(),
            ThemeMode::Dark => dark.name(),
        })
        .size(11),
    ]
    .spacing(2)
    .align_x(Alignment::Center);

    button(
        column![preview, label]
            .spacing(10)
            .align_x(Alignment::Center),
    )
    .padding(10)
    .on_press(Message::WelcomeChoose(mode))
    .style(move |theme: &Theme, status| {
        let palette = theme.extended_palette();
        let border_color = if is_selected {
            palette.primary.base.color
        } else if matches!(status, button::Status::Hovered) {
            palette.background.strong.color
        } else {
            palette.background.weak.color
        };
        button::Style {
            background: Some(palette.background.weakest.color.into()),
            text_color: palette.background.base.text,
            border: Border {
                color: border_color,
                width: 2.0,
                radius: 10.0.into(),
            },
            ..button::Style::default()
        }
    })
    .into()
}

/// Miniatura de código con los colores de `theme`.
fn preview(theme: &ActiveTheme, width: f32) -> Element<'static, Message> {
    let colors = &theme.editor;
    let background = colors.background;
    let gutter = colors.gutter;
    let foreground = colors.text;
    let numbers = colors.line_number;
    let accent = colors
        .style(Some(lorca_syntax::HighlightKind::String))
        .color
        .unwrap_or(foreground);

    let lines = SAMPLE.iter().enumerate().map(|(index, line)| {
        let color = if index == 1 { accent } else { foreground };
        row![
            container(
                text((index + 1).to_string())
                    .size(10)
                    .font(Font::MONOSPACE)
                    .color(numbers)
            )
            .width(18)
            .style(move |_: &Theme| container::Style::default().background(gutter)),
            text(*line)
                .size(10)
                .font(Font::MONOSPACE)
                .color(color)
                .wrapping(text::Wrapping::None),
        ]
        .spacing(6)
        .into()
    });

    container(column(lines).spacing(3))
        .padding(8)
        .width(width)
        .height(PREVIEW_HEIGHT)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            background: Some(background.into()),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}
