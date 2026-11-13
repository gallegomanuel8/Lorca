//! Ventana «Acerca de Lorca» para Windows y Linux (en macOS la da el sistema, desde el menú
//! de la aplicación): el retrato de Lorca hecho con caracteres, la versión y un enlace a
//! la página del proyecto. Con los colores del tema, como las demás ventanas propias.
//! Escape o «Cerrar» la cierran.

use std::sync::OnceLock;

use iced::widget::{
    Space, button, center, column, container, image, mouse_area, opaque, row, stack, text,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Shadow, Theme, Vector};

use crate::app::Message;

/// Página del proyecto.
pub const PROJECT_URL: &str = "https://github.com/gallegomanuel8/Lorca";

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
            row![
                button(text("Página del proyecto").size(13))
                    .padding([5, 14])
                    .style(button::secondary)
                    .on_press(Message::OpenUrl(PROJECT_URL.to_owned())),
                button(text("Cerrar").size(13))
                    .padding([5, 14])
                    .style(button::primary)
                    .on_press(Message::CloseAbout),
            ]
            .spacing(8),
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
