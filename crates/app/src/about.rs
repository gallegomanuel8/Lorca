//! Ventana «Acerca de Lorca» para Windows y Linux (en macOS la da el sistema, desde el menú
//! de la aplicación): el retrato de Lorca hecho con caracteres, la versión y accesos
//! directos al repositorio de GitHub (código, novedades y avisar de un problema). Con los colores del tema, como las demás ventanas propias.
//! Escape o «Cerrar» la cierran.

use std::sync::OnceLock;

use iced::widget::{
    Space, button, center, column, container, image, mouse_area, opaque, row, stack, svg, text,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Shadow, Theme, Vector};

use crate::app::Message;

/// Repositorio del proyecto.
pub const PROJECT_URL: &str = "https://github.com/gallegomanuel8/Lorca";

const GITHUB: &[u8] = include_bytes!("../icons/github.svg");
const TAG: &[u8] = include_bytes!("../icons/tag.svg");
const BUG: &[u8] = include_bytes!("../icons/bug.svg");

/// Accesos directos: icono, título, explicación y dirección (relativa al repositorio).
const LINKS: [(&[u8], &str, &str, &str); 3] = [
    (GITHUB, "Repositorio en GitHub", "El código de Lorca", ""),
    (
        TAG,
        "Novedades",
        "Todas las versiones publicadas",
        "/releases",
    ),
    (
        BUG,
        "Avisar de un problema",
        "O proponer una mejora",
        "/issues/new",
    ),
];

/// Retrato (se carga una vez: un `Handle` nuevo en cada `view` volvería a subirlo).
fn portrait() -> image::Handle {
    static PORTRAIT: OnceLock<image::Handle> = OnceLock::new();
    PORTRAIT
        .get_or_init(|| {
            image::Handle::from_bytes(
                include_bytes!("../../../assets/icons/retrato/lorca-256.png").as_slice(),
            )
        })
        .clone()
}

/// Lista de accesos directos al repositorio, en un recuadro.
fn links() -> Element<'static, Message> {
    let rows = LINKS.iter().map(|(icon, title, detail, path)| {
        let icon = svg(svg::Handle::from_memory(*icon))
            .width(20)
            .height(20)
            .style(|theme: &Theme, _| svg::Style {
                color: Some(theme.extended_palette().primary.base.color),
            });
        let label = column![
            text(*title).size(13).font(Font {
                weight: iced::font::Weight::Semibold,
                ..Font::default()
            }),
            text(*detail).size(11).style(|theme: &Theme| text::Style {
                color: Some(Color {
                    a: 0.65,
                    ..theme.extended_palette().background.base.text
                }),
            }),
        ]
        .spacing(1);
        button(
            row![
                icon,
                label,
                Space::new().width(Length::Fill),
                text("↗").size(13)
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([7, 12])
        .width(Length::Fill)
        .style(|theme: &Theme, status| {
            let palette = theme.extended_palette();
            let background = match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(palette.background.weak.color.into())
                }
                _ => None,
            };
            button::Style {
                background,
                text_color: palette.background.base.text,
                border: Border {
                    radius: 6.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }
        })
        .on_press(Message::OpenUrl(format!("{PROJECT_URL}{path}")))
        .into()
    });
    container(column(rows).spacing(2))
        .padding(4)
        .width(Length::Fill)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style::default().border(Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 8.0.into(),
            })
        })
        .into()
}

/// Dibuja la ventana centrada sobre `base`, con el fondo oscurecido.
pub fn layer<'a>(base: Element<'a, Message>) -> Element<'a, Message> {
    let muted = |theme: &Theme| text::Style {
        color: Some(Color {
            a: 0.7,
            ..theme.extended_palette().background.base.text
        }),
    };
    let card = container(
        column![
            image(portrait()).width(128).height(128),
            Space::new().height(4),
            text("Lorca").size(24).font(Font {
                weight: iced::font::Weight::Semibold,
                ..Font::default()
            }),
            text(format!("Versión {}", env!("CARGO_PKG_VERSION")))
                .size(13)
                .style(muted),
            text("Verde que te quiero verde.").size(14).font(Font {
                style: iced::font::Style::Italic,
                ..Font::default()
            }),
            Space::new().height(4),
            text("Editor de texto y código, ligero y libre.")
                .size(13)
                .style(muted),
            Space::new().height(10),
            links(),
            Space::new().height(10),
            button(text("Cerrar").size(13))
                .padding([5, 22])
                .style(button::primary)
                .on_press(Message::CloseAbout),
        ]
        .spacing(4)
        .width(Length::Fill)
        .align_x(Alignment::Center),
    )
    .padding(24)
    .width(Length::Fixed(340.0))
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style::default()
            .background(palette.background.base.color)
            .color(palette.background.base.text)
            .border(Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 10.0.into(),
            })
            .shadow(Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 24.0,
            })
    });

    let backdrop = container(center(opaque(card)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_theme: &Theme| {
            container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.35))
        });
    // Un clic fuera de la ventana también la cierra.
    stack![
        base,
        opaque(mouse_area(backdrop).on_press(Message::CloseAbout))
    ]
    .into()
}
