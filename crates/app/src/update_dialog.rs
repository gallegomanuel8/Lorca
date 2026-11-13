//! Ventana del aviso de versión nueva: tarjeta centrada con el icono de Lorca, la versión
//! actual y la nueva, y los botones «Más tarde» y «Actualizar y reiniciar». Mientras se
//! descarga e instala, los botones dejan paso a una barra de progreso. Usa la letra y los
//! colores del tema, como las ventanas de confirmación (`prompt.rs`).
//!
//! Escape equivale a «Más tarde». Intro no instala: el aviso puede salir al arrancar,
//! mientras se escribe, y no debe reiniciar Lorca por una tecla pulsada sin querer.

use std::sync::OnceLock;

use iced::widget::{
    button, center, column, container, image, mouse_area, opaque, progress_bar, rich_text, row,
    rule, space, span, stack, text,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Shadow, Theme, Vector};

use crate::app::Message;
use crate::updates::{self, Available};

/// Anchura de la tarjeta.
const WIDTH: f32 = 460.0;

/// Lo que muestra la ventana.
pub enum View<'a> {
    Available(&'a Available),
    Working {
        version: &'a str,
        progress: &'a updates::Progress,
    },
}

/// Icono de la aplicación (se carga una vez: un `Handle` nuevo en cada `view` volvería a
/// subir la imagen a la GPU).
pub(crate) fn icon() -> image::Handle {
    static ICON: OnceLock<image::Handle> = OnceLock::new();
    ICON.get_or_init(|| {
        image::Handle::from_bytes(
            include_bytes!("../../../assets/icons/png/lorca-128.png").as_slice(),
        )
    })
    .clone()
}

/// Texto secundario (atenuado) del tema.
fn muted(theme: &Theme) -> text::Style {
    let palette = theme.extended_palette();
    text::Style {
        color: Some(Color {
            a: 0.7,
            ..palette.background.base.text
        }),
    }
}

/// Dibuja el aviso centrado sobre `base`, con el fondo oscurecido y sin dejar pulsar nada
/// de debajo.
pub fn layer<'a>(base: Element<'a, Message>, view: View<'a>) -> Element<'a, Message> {
    let version = match &view {
        View::Available(available) => available.version.as_str(),
        View::Working { version, .. } => version,
    }
    .trim_start_matches('v');
    let working = matches!(view, View::Working { .. });

    let title = row![
        text("Lorca").size(16).font(Font {
            weight: iced::font::Weight::Semibold,
            ..Font::default()
        }),
        badge(),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let versions = text(format!("{} → {version}", env!("CARGO_PKG_VERSION")))
        .size(12)
        .style(muted);
    let mut close = button(text("✕").size(13))
        .padding([2, 6])
        .style(button::text);
    if !working {
        close = close.on_press(Message::DismissUpdate);
    }
    let header = row![
        image(icon()).width(44).height(44),
        column![title, versions].spacing(2),
        space::horizontal(),
        close,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let body = text(format!(
        "Lorca {version} ya está disponible, con las últimas novedades y correcciones."
    ))
    .size(13);

    let footer: Element<'a, Message> = match view {
        View::Available(available) => {
            let action = if available.installable {
                "Actualizar y reiniciar"
            } else {
                "Descargar"
            };
            row![
                button(rich_text![span::<(), _>("Novedades").underline(true)].size(13))
                    .padding(0)
                    .style(link_style)
                    .on_press(Message::OpenUpdatePage),
                space::horizontal(),
                button(text("Más tarde").size(13))
                    .padding([5, 14])
                    .style(button::secondary)
                    .on_press(Message::DismissUpdate),
                button(text(action).size(13))
                    .padding([5, 14])
                    .style(button::primary)
                    .on_press(Message::InstallUpdate),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .into()
        }
        View::Working { progress, .. } => {
            let (label, fraction) = match progress {
                updates::Progress::Downloading(Some(fraction)) => {
                    (format!("Descargando… {:.0} %", fraction * 100.0), *fraction)
                }
                updates::Progress::Downloading(None) => ("Descargando…".to_owned(), 0.0),
                _ => ("Instalando…".to_owned(), 1.0),
            };
            row![
                text(label).size(13),
                space::horizontal(),
                progress_bar(0.0..=1.0, fraction)
                    .length(Length::Fixed(240.0))
                    .girth(Length::Fixed(6.0)),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }
    };

    let card = container(
        column![header, body, rule::horizontal(1), footer]
            .spacing(12)
            .width(Length::Fill),
    )
    .padding(20)
    .width(Length::Fixed(WIDTH))
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
    stack![base, opaque(mouse_area(backdrop))].into()
}

/// Etiqueta «Actualización» con el color principal del tema.
fn badge<'a>() -> Element<'a, Message> {
    container(text("Actualización").size(11))
        .padding([1, 7])
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style::default()
                .background(palette.primary.base.color)
                .color(palette.primary.base.text)
                .border(Border {
                    radius: 4.0.into(),
                    ..Border::default()
                })
        })
        .into()
}

/// «Novedades»: texto subrayado, como un enlace (color principal al pasar el ratón).
fn link_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => palette.primary.base.color,
        _ => palette.background.base.text,
    };
    button::Style {
        background: None,
        text_color,
        ..button::Style::default()
    }
}
