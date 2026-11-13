//! Pantalla de bienvenida del primer arranque: elección del tema y, después, un resumen
//! de lo que hace Lorca (`tour`). Se vuelve a enseñar cuando sube
//! `lorca_workspace::WELCOME_VERSION`.

use iced::theme::Mode;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Theme};
use lorca_workspace::{Appearance, Config, ThemeMode, ThemeSet};

use crate::app::Message;
use crate::shortcut_labels as keys;
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

/// Una página del resumen de funciones: título, frase y puntos (atajo o etiqueta, texto).
struct TourPage {
    title: &'static str,
    intro: &'static str,
    items: &'static [(&'static str, &'static str)],
}

/// Resumen de funciones que se enseña tras elegir el tema.
const TOUR: [TourPage; 5] = [
    TourPage {
        title: "Tres maneras de escribir",
        intro: "Cada pestaña es de un tipo; se cambia desde la barra de estado.",
        items: &[
            ("Texto plano", "Tal cual, sin formato ni ajuste de línea."),
            (
                "Texto con formato",
                "Títulos, negrita, listas, tablas y vínculos se ven mientras escribes. \
                 Por debajo es Markdown, así que se abre en cualquier otro editor.",
            ),
            (
                "Código",
                "Resaltado de sintaxis para decenas de lenguajes. Al pegar en un documento \
                 nuevo, Lorca adivina el lenguaje.",
            ),
            (
                "Barra de formato",
                "Encima de las pestañas: encabezados, listas, negrita, vínculos, tablas e \
                 imágenes.",
            ),
        ],
    },
    TourPage {
        title: "Imágenes en tus notas",
        intro: "En el texto con formato las imágenes se ven dentro del documento.",
        items: &[
            (
                "Insertar",
                "Con el botón de la barra, pegando una captura o arrastrando fotos a la \
                 ventana.",
            ),
            (
                "Ordenar",
                "Un clic la selecciona; arrástrala para moverla y usa el tirador de su \
                 esquina para cambiar su tamaño.",
            ),
            (
                "Junto al documento",
                "Se guardan en la carpeta «<nombre>_imagenes». Las fotos muy pesadas se \
                 reducen para que todo vaya ligero.",
            ),
        ],
    },
    TourPage {
        title: "Buscar y editar rápido",
        intro: "Todo con el teclado, sin quitar las manos del texto.",
        items: &[
            (
                keys::FIND,
                "Buscar y reemplazar, también con expresiones regulares.",
            ),
            (
                keys::FIND_IN_FOLDER,
                "Buscar en todos los archivos de una carpeta.",
            ),
            (keys::GOTO, "Ir a una línea."),
            (
                MULTI_CURSOR,
                "Varios cursores a la vez: escribe en muchos sitios de golpe.",
            ),
            (
                keys::BOOKMARK,
                "Marcadores para volver a un punto del documento.",
            ),
        ],
    },
    TourPage {
        title: "Para programar",
        intro: "Herramientas de editor de código, sin perder ligereza.",
        items: &[
            (
                keys::PALETTE,
                "Paleta de comandos: cualquier acción, escribiendo su nombre.",
            ),
            (
                keys::SPLIT,
                "Pantalla dividida para ver varios documentos a la vez.",
            ),
            (
                keys::COMPLETE,
                "Autocompletado, ir a la definición (F12) y errores (F8) con los \
                 servidores de lenguaje que tengas instalados.",
            ),
            (keys::FORMAT, "Formatear JSON y SQL."),
        ],
    },
    TourPage {
        title: "Lorca recuerda por ti",
        intro: "Cierra cuando quieras: al volver todo sigue donde estaba.",
        items: &[
            (
                "Sesión",
                "Las pestañas abiertas y los cambios sin guardar se recuperan al abrir \
                 Lorca.",
            ),
            (
                ZOOM,
                "Zoom en cada pestaña (también con la rueda o pellizcando el panel \
                 táctil).",
            ),
            ("Temas", "Claros y oscuros, desde la barra de estado."),
            ("Exportar", "Los textos con formato, a PDF o HTML."),
            (
                "Al día",
                "Lorca avisa cuando hay una versión nueva y se actualiza con un clic.",
            ),
        ],
    },
];

/// Atajo de varios cursores.
const MULTI_CURSOR: &str = if cfg!(target_os = "macos") {
    "⌘D · ⌥+clic"
} else {
    "Ctrl+D · Alt+clic"
};
/// Atajos del zoom.
const ZOOM: &str = if cfg!(target_os = "macos") {
    "⌘+ · ⌘−"
} else {
    "Ctrl++ · Ctrl+−"
};

/// Número de páginas del resumen.
pub const TOUR_PAGES: usize = TOUR.len();

/// Página `page` del resumen de funciones.
pub fn tour(page: usize) -> Element<'static, Message> {
    let page = page.min(TOUR_PAGES - 1);
    let Some(content) = TOUR.get(page) else {
        return Space::new().into();
    };
    let semibold = Font {
        weight: iced::font::Weight::Semibold,
        ..Font::default()
    };
    let items = column(content.items.iter().map(|(label, description)| {
        row![
            container(text(*label).size(13).font(semibold))
                .width(Length::Fixed(170.0))
                .padding([1, 0]),
            text(*description).size(14).width(Length::Fill),
        ]
        .spacing(16)
        .into()
    }))
    .spacing(14);

    let dots = row((0..TOUR_PAGES).map(|index| {
        let current = index == page;
        container(Space::new().width(8).height(8))
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style::default()
                    .background(if current {
                        palette.primary.base.color
                    } else {
                        palette.background.strong.color
                    })
                    .border(Border {
                        radius: 4.0.into(),
                        ..Border::default()
                    })
            })
            .into()
    }))
    .spacing(8);

    let last = page + 1 == TOUR_PAGES;
    let mut buttons = row![
        button(text("Saltar").size(13))
            .padding([6, 12])
            .style(button::text)
            .on_press(Message::TourDone),
        Space::new().width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if page > 0 {
        buttons = buttons.push(
            button(text("Anterior").size(14))
                .padding([8, 18])
                .style(button::secondary)
                .on_press(Message::TourPage(page - 1)),
        );
    }
    buttons = buttons.push(
        button(
            text(if last {
                "Empezar a escribir"
            } else {
                "Siguiente"
            })
            .size(14),
        )
        .padding([8, 24])
        .style(button::primary)
        .on_press(if last {
            Message::TourDone
        } else {
            Message::TourPage(page + 1)
        }),
    );

    let body = column![
        text(format!(
            "Lo que puedes hacer con Lorca · {} de {TOUR_PAGES}",
            page + 1
        ))
        .size(12)
        .style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().primary.base.color),
        }),
        text(content.title).size(28),
        text(content.intro).size(14).font(Font {
            style: iced::font::Style::Italic,
            ..Font::default()
        }),
        Space::new().height(10),
        items,
        Space::new().height(Length::Fill),
        container(dots).center_x(Length::Fill),
        Space::new().height(12),
        buttons,
    ]
    .spacing(8)
    .width(Length::Fixed(640.0))
    .height(Length::Fixed(470.0));

    container(body).center(Length::Fill).into()
}
