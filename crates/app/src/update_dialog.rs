//! Ventana del aviso de versión nueva: tarjeta centrada con el icono de Lorca, la versión
//! actual y la nueva, y los botones «Más tarde» y «Actualizar y reiniciar». Mientras se
//! descarga e instala, los botones dejan paso a una barra de progreso.
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
const WIDTH: f32 = 500.0;
/// Rojo de Lorca (la luna del icono): insignia y botón principal.
const LORCA_RED: Color = Color::from_rgb(0.70, 0.15, 0.18);

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
fn icon() -> image::Handle {
    static ICON: OnceLock<image::Handle> = OnceLock::new();
    ICON.get_or_init(|| {
        image::Handle::from_bytes(
            include_bytes!("../../../assets/icons/png/lorca-128.png").as_slice(),
        )
    })
    .clone()
}

/// Dibuja el aviso centrado sobre `base`, con el fondo algo oscurecido y sin dejar pulsar
/// nada de debajo.
pub fn layer<'a>(base: Element<'a, Message>, view: View<'a>) -> Element<'a, Message> {
    let version = match &view {
        View::Available(available) => available.version.as_str(),
        View::Working { version, .. } => version,
    }
    .trim_start_matches('v');
    let working = matches!(view, View::Working { .. });

    let title = row![
        text("Lorca").size(18),
        text("editor de texto y código")
            .size(13)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().background.strong.text),
            }),
        badge(),
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let versions = text(format!("v{} → {version}", env!("CARGO_PKG_VERSION")))
        .size(13)
        .font(Font::MONOSPACE);
    let mut close = button(text("✕").size(14))
        .padding([2, 6])
        .style(button::text);
    if !working {
        close = close.on_press(Message::DismissUpdate);
    }
    let header = row![
        image(icon()).width(48).height(48),
        column![title, versions].spacing(4),
        space::horizontal(),
        close,
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let body = text(format!(
        "Lorca {version} ya está disponible, con las últimas novedades y correcciones."
    ))
    .size(14);

    let footer: Element<'a, Message> = match view {
        View::Available(available) => {
            let action = if available.installable {
                "ACTUALIZAR Y REINICIAR"
            } else {
                "DESCARGAR"
            };
            row![
                button(rich_text![span::<(), _>("Novedades").underline(true)].size(13))
                    .padding(0)
                    .style(link_style)
                    .on_press(Message::OpenUpdatePage),
                space::horizontal(),
                button(text("MÁS TARDE").size(12).font(Font::MONOSPACE))
                    .padding([7, 14])
                    .style(later_style)
                    .on_press(Message::DismissUpdate),
                button(text(action).size(12).font(Font::MONOSPACE))
                    .padding([7, 14])
                    .style(update_style)
                    .on_press(Message::InstallUpdate),
            ]
            .spacing(12)
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
                    .length(Length::Fixed(260.0))
                    .girth(Length::Fixed(6.0))
                    .style(|theme: &Theme| progress_bar::Style {
                        bar: LORCA_RED.into(),
                        ..progress_bar::primary(theme)
                    }),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
        }
    };

    let card = container(
        column![header, body, rule::horizontal(1), footer]
            .spacing(16)
            .width(Length::Fill),
    )
    .padding(22)
    .width(Length::Fixed(WIDTH))
    .style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style::default()
            .background(palette.background.base.color)
            .color(palette.background.base.text)
            .border(Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 6.0.into(),
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
            container::Style::default().background(Color::from_rgba(0.0, 0.0, 0.0, 0.2))
        });
    stack![base, opaque(mouse_area(backdrop))].into()
}

/// Insignia roja «ACTUALIZACIÓN».
fn badge<'a>() -> Element<'a, Message> {
    container(
        text("ACTUALIZACIÓN")
            .size(11)
            .font(Font::MONOSPACE)
            .color(Color::WHITE),
    )
    .padding([2, 6])
    .style(|_theme: &Theme| {
        container::Style::default()
            .background(LORCA_RED)
            .border(Border {
                radius: 2.0.into(),
                ..Border::default()
            })
    })
    .into()
}

/// «Novedades»: texto subrayado, como un enlace (rojo al pasar el ratón).
fn link_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let text_color = match status {
        button::Status::Hovered | button::Status::Pressed => LORCA_RED,
        _ => palette.background.base.text,
    };
    button::Style {
        background: None,
        text_color,
        ..button::Style::default()
    }
}

/// «Más tarde»: solo el borde.
fn later_style(theme: &Theme, status: button::Status) -> button::Style {
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
            color: palette.background.strong.color,
            width: 1.0,
            radius: 2.0.into(),
        },
        ..button::Style::default()
    }
}

/// «Actualizar y reiniciar»: relleno rojo.
fn update_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered | button::Status::Pressed => Color {
            a: 0.85,
            ..LORCA_RED
        },
        _ => LORCA_RED,
    };
    button::Style {
        background: Some(background.into()),
        text_color: Color::WHITE,
        border: Border {
            color: LORCA_RED,
            width: 1.0,
            radius: 2.0.into(),
        },
        ..button::Style::default()
    }
}
