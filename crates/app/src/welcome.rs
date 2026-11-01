//! Pantalla de bienvenida del primer arranque: elección del tema.

use iced::theme::Mode;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme};
use lorca_workspace::ThemeMode;

use crate::app::{Message, resolve_theme};

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
pub fn view(selected: ThemeMode, system: Mode) -> Element<'static, Message> {
    let cards = row(ThemeMode::ALL.map(|mode| card(mode, mode == selected, system))).spacing(16);

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
fn card(mode: ThemeMode, is_selected: bool, system: Mode) -> Element<'static, Message> {
    let preview: Element<'static, Message> = match mode {
        // En automático se muestran ambos, mitad y mitad.
        ThemeMode::Auto => row![
            preview(&Theme::Light, PREVIEW_WIDTH / 2.0),
            preview(&Theme::Dark, PREVIEW_WIDTH / 2.0)
        ]
        .into(),
        _ => preview(&resolve_theme(mode, system), PREVIEW_WIDTH),
    };
    let label = column![
        text(mode.label()).size(14),
        text(match mode {
            ThemeMode::Auto => "Sigue al sistema",
            ThemeMode::Light => "Fondo claro",
            ThemeMode::Dark => "Fondo oscuro",
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
fn preview(theme: &Theme, width: f32) -> Element<'static, Message> {
    let palette = theme.extended_palette();
    let background = palette.background.base.color;
    let gutter = palette.background.weakest.color;
    let foreground = palette.background.base.text;
    let numbers = palette.background.strong.color;
    let accent = palette.primary.base.color;

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
