//! Resumen de funciones que se enseña tras elegir el tema en la bienvenida (y desde «Lo que
//! puedes hacer con Lorca»). Cada página tiene una ilustración a la izquierda, dibujada con
//! widgets y los colores del tema activo (parece una captura de Lorca, pero no pesa nada y
//! siempre coincide con el tema elegido), y a la derecha poco texto con los atajos.
//!
//! ←/→ cambian de página, Intro avanza y Escape lo cierra.

use iced::widget::{Space, button, column, container, image, rich_text, row, span, stack, text};
use iced::{Alignment, Border, Color, Element, Font, Length, Padding, Shadow, Theme, Vector};
use lorca_syntax::HighlightKind;

use crate::app::Message;
use crate::editor::palette::EditorColors;
use crate::shortcut_labels as keys;

/// Número de páginas.
pub const PAGES: usize = 5;

/// Letra de los textos con formato (la del editor WYSIWYG).
const PROSE: Font = Font {
    family: crate::editor::PROSE_FAMILY,
    ..Font::DEFAULT
};

/// Tamaño de las ilustraciones.
const SHOT_WIDTH: f32 = 400.0;
const SHOT_HEIGHT: f32 = 270.0;

/// Página `page` del resumen con los colores del editor `colors`.
pub fn view(page: usize, colors: &EditorColors) -> Element<'static, Message> {
    let page = page.min(PAGES - 1);
    let (title, intro, points, shot): (&str, &str, Vec<Point>, Element<'static, Message>) =
        match page {
            0 => (
                "Escribe como quieras",
                "Cada pestaña puede ser texto plano, texto con formato o código.",
                vec![
                    Point::text("Elige el tipo en la barra de estado, abajo a la derecha."),
                    Point::text(
                        "Con formato, los títulos, listas y negritas se ven mientras escribes.",
                    ),
                    Point::keys(
                        &[keys::BOLD, keys::ITALIC, keys::LINK],
                        "negrita, cursiva, vínculo",
                    ),
                ],
                writing(colors),
            ),
            1 => (
                "Imágenes en tus notas",
                "Pega una captura o arrastra una foto al documento.",
                vec![
                    Point::text("Haz clic en una imagen para seleccionarla."),
                    Point::text(
                        "Arrástrala para moverla; el tirador de la esquina cambia su tamaño.",
                    ),
                    Point::text(
                        "Se guardan en una carpeta junto al documento, reducidas si pesan mucho.",
                    ),
                ],
                images(colors),
            ),
            2 => (
                "Encuentra cualquier cosa",
                "Buscar, reemplazar y escribir en varios sitios a la vez.",
                vec![
                    Point::keys(&[keys::FIND], "buscar y reemplazar"),
                    Point::keys(&[keys::FIND_IN_FOLDER], "buscar en una carpeta"),
                    Point::keys(&[MULTI_CURSOR], "varios cursores"),
                ],
                search(colors),
            ),
            3 => (
                "Para programar",
                "Resaltado de código y todas las órdenes a mano.",
                vec![
                    Point::keys(&[keys::PALETTE], "paleta de comandos"),
                    Point::keys(&[keys::SPLIT], "pantalla dividida"),
                    Point::keys(&[keys::COMPLETE, keys::DEFINITION], "completar, definición"),
                ],
                coding(colors),
            ),
            _ => (
                "Lorca lo recuerda todo",
                "Cierra cuando quieras: al volver, todo sigue donde estaba.",
                vec![
                    Point::text("Las pestañas y los cambios sin guardar se recuperan."),
                    Point::keys(&[keys::ZOOM_IN, keys::ZOOM_OUT], "zoom de cada pestaña"),
                    Point::text("Avisa cuando hay una versión nueva y se actualiza con un clic."),
                ],
                memory(colors),
            ),
        };

    let points = column(points.into_iter().map(Point::view)).spacing(14);
    let side = column![
        text(format!("{} de {PAGES}", page + 1))
            .size(12)
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().primary.base.color),
            }),
        text(title).size(26),
        text(intro).size(14).style(muted),
        Space::new().height(12),
        points,
    ]
    .spacing(6)
    .width(Length::Fixed(300.0));

    let body = column![
        row![shot, side].spacing(36).align_y(Alignment::Center),
        Space::new().height(28),
        footer(page),
    ]
    .width(Length::Fixed(SHOT_WIDTH + 36.0 + 300.0));
    container(body).center(Length::Fill).into()
}

/// Atajo de varios cursores.
const MULTI_CURSOR: &str = if cfg!(target_os = "macos") {
    "⌥+clic"
} else {
    "Alt+clic"
};

/// Un punto del texto de la derecha: teclas (opcionales) y explicación.
struct Point {
    keys: &'static [&'static str],
    text: &'static str,
}

impl Point {
    fn text(text: &'static str) -> Self {
        Self { keys: &[], text }
    }

    fn keys(keys: &'static [&'static str], text: &'static str) -> Self {
        Self { keys, text }
    }

    fn view(self) -> Element<'static, Message> {
        if self.keys.is_empty() {
            return row![text("•").size(14).style(accent), text(self.text).size(14)]
                .spacing(8)
                .into();
        }
        let mut line = row![].spacing(6).align_y(Alignment::Center);
        for key in self.keys {
            line = line.push(keycap(key));
        }
        line.push(Space::new().width(4))
            .push(text(self.text).size(14))
            .wrap()
            .into()
    }
}

/// Tecla dibujada como una tecla de verdad.
fn keycap(label: &'static str) -> Element<'static, Message> {
    container(text(label).size(12).font(Font::MONOSPACE))
        .padding([2, 7])
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style::default()
                .background(palette.background.weak.color)
                .color(palette.background.base.text)
                .border(Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 5.0.into(),
                })
                .shadow(Shadow {
                    color: palette.background.strong.color,
                    offset: Vector::new(0.0, 1.5),
                    blur_radius: 0.0,
                })
        })
        .into()
}

/// Puntos de página, «Saltar», «Anterior» y «Siguiente».
fn footer(page: usize) -> Element<'static, Message> {
    let dots = row((0..PAGES).map(|index| {
        let current = index == page;
        button(Space::new().width(if current { 22 } else { 8 }).height(8))
            .padding(0)
            .on_press(Message::TourPage(index))
            .style(move |theme: &Theme, _| {
                let palette = theme.extended_palette();
                button::Style {
                    background: Some(
                        if current {
                            palette.primary.base.color
                        } else {
                            palette.background.strong.color
                        }
                        .into(),
                    ),
                    border: Border {
                        radius: 4.0.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .into()
    }))
    .spacing(6)
    .align_y(Alignment::Center);

    let last = page + 1 == PAGES;
    let mut buttons = row![].spacing(8).align_y(Alignment::Center);
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
    row![
        button(text("Saltar").size(13))
            .padding([6, 0])
            .style(button::text)
            .on_press(Message::TourDone),
        Space::new().width(Length::Fill),
        dots,
        Space::new().width(Length::Fill),
        buttons,
    ]
    .align_y(Alignment::Center)
    .into()
}

// --- Ilustraciones ------------------------------------------------------------------

/// Marco de una ilustración: una ventana de Lorca en miniatura.
fn shot<'a>(colors: &EditorColors, content: Element<'a, Message>) -> Element<'a, Message> {
    let background = colors.background;
    let border = colors.gutter;
    container(content)
        .width(Length::Fixed(SHOT_WIDTH))
        .height(Length::Fixed(SHOT_HEIGHT))
        .clip(true)
        .style(move |_: &Theme| {
            container::Style::default()
                .background(background)
                .border(Border {
                    color: border,
                    width: 1.0,
                    radius: 10.0.into(),
                })
                .shadow(Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                    offset: Vector::new(0.0, 10.0),
                    blur_radius: 30.0,
                })
        })
        .into()
}

/// Franja (barra de herramientas, pestañas, estado) con el fondo del margen.
fn strip<'a>(
    colors: &EditorColors,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let background = colors.gutter;
    container(content)
        .padding([5, 10])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style::default().background(background))
        .into()
}

/// Texto de la ilustración en un color.
fn ink(content: &'static str, size: u16, color: Color) -> iced::widget::Text<'static> {
    text(content).size(f32::from(size)).color(color).font(PROSE)
}

/// Página 1: barra de formato y un texto con formato.
fn writing(colors: &EditorColors) -> Element<'static, Message> {
    let c = colors.clone();
    let tool = |label: &'static str, font: Font| text(label).size(13).font(font).color(c.text);
    let bold = Font {
        weight: iced::font::Weight::Bold,
        ..PROSE
    };
    let italic = Font {
        style: iced::font::Style::Italic,
        ..PROSE
    };
    let toolbar = strip(
        colors,
        row![
            tool("H1 ▾", Font::default()),
            tool("☰ ▾", Font::default()),
            tool("B", bold),
            tool("I", italic),
            tool("S", Font::default()),
            tool("🔗", Font::default()),
            tool("🖼", Font::default()),
            tool("▦", Font::default()),
        ]
        .spacing(14),
    );
    let page = column![
        ink("Viaje a Granada", 22, c.text).font(bold),
        rich_text![
            sp("Subimos a la "),
            sp("Alhambra").font(bold),
            sp(" al atardecer, con la luz "),
            sp("color de membrillo").font(italic),
            sp("."),
        ]
        .size(13)
        .font(PROSE)
        .color(c.text),
        column![
            ink("•  Generalife y sus fuentes", 13, c.text),
            ink("•  Mirador de San Nicolás", 13, c.text),
            ink("•  Cena en el Albaicín", 13, c.text),
        ]
        .spacing(4),
        rich_text![
            sp("Más fotos en "),
            sp("el álbum").underline(true).color(c.bookmark)
        ]
        .size(13)
        .font(PROSE)
        .color(c.text),
    ]
    .spacing(10)
    .padding(18);
    let status = strip(
        colors,
        row![
            Space::new().width(Length::Fill),
            container(ink("Texto con formato ▾", 11, c.text))
                .padding([1, 6])
                .style(highlight(c.selection)),
        ],
    );
    shot(
        colors,
        column![toolbar, page, Space::new().height(Length::Fill), status].into(),
    )
}

/// Página 2: una imagen seleccionada con su tirador.
fn images(colors: &EditorColors) -> Element<'static, Message> {
    let c = colors.clone();
    let picture = container(image(crate::update_dialog::icon()).width(120).height(120))
        .padding(2)
        .style(move |_: &Theme| {
            container::Style::default().border(Border {
                color: c.cursor,
                width: 2.0,
                radius: 0.0.into(),
            })
        });
    let handle = container(Space::new().width(10).height(10)).style({
        let (cursor, background) = (colors.cursor, colors.background);
        move |_: &Theme| {
            container::Style::default()
                .background(cursor)
                .border(Border {
                    color: background,
                    width: 1.0,
                    radius: 0.0.into(),
                })
        }
    });
    let selected = stack![
        picture,
        container(handle)
            .width(Length::Fixed(129.0))
            .height(Length::Fixed(129.0))
            .align_right(Length::Fixed(129.0))
            .align_bottom(Length::Fixed(129.0)),
    ];
    let page = column![
        ink("Nuestro logo", 18, c.text).font(Font {
            weight: iced::font::Weight::Bold,
            ..PROSE
        }),
        ink("Esta es la versión definitiva:", 13, c.text),
        selected,
        ink("Luna roja entre renglones", 11, c.line_number).font(Font {
            style: iced::font::Style::Italic,
            ..PROSE
        }),
    ]
    .spacing(8)
    .padding(18);
    let hint = container(
        ink("⤢ arrastra para cambiar el tamaño", 11, c.background)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([3, 8])
    .style({
        let cursor = colors.cursor;
        move |_: &Theme| {
            container::Style::default()
                .background(cursor)
                .border(Border {
                    radius: 10.0.into(),
                    ..Border::default()
                })
        }
    });
    shot(
        colors,
        stack![
            container(page).width(Length::Fill).height(Length::Fill),
            container(hint).padding(Padding::default().top(178).left(158))
        ]
        .into(),
    )
}

/// Página 3: barra de búsqueda y coincidencias resaltadas.
fn search(colors: &EditorColors) -> Element<'static, Message> {
    let c = colors.clone();
    let field = container(ink("luna", 12, c.text))
        .padding([3, 8])
        .width(Length::Fixed(170.0))
        .style(move |_: &Theme| {
            container::Style::default()
                .background(c.background)
                .border(Border {
                    color: c.cursor,
                    width: 1.0,
                    radius: 4.0.into(),
                })
        });
    let bar = strip(
        colors,
        row![
            ink("Buscar", 12, c.text),
            field,
            ink("2 de 4", 11, c.line_number),
            Space::new().width(Length::Fill),
            ink("Aa", 11, c.line_number),
            ink(".*", 11, c.line_number),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    );
    let found = |word: &'static str, current: bool| {
        sp(word).background(if current { c.found_current } else { c.found })
    };
    let line = |spans: Vec<iced::widget::text::Span<'static, ()>>| {
        rich_text(spans)
            .size(13)
            .color(c.text)
            .font(Font::MONOSPACE)
    };
    let lines = column![
        line(vec![
            sp("La "),
            found("luna", false),
            sp(" vino a la fragua")
        ]),
        line(vec![sp("con su polisón de nardos.")]),
        line(vec![sp("El niño la mira, mira,")]),
        line(vec![sp("el niño la está mirando.")]),
        line(vec![
            sp("Huye "),
            found("luna", true),
            sp(", "),
            found("luna", false),
            sp(", "),
            found("luna", false),
            sp(".")
        ]),
        line(vec![sp("Si vinieran los gitanos,")]),
        line(vec![sp("harían con tu corazón")]),
        line(vec![sp("collares y anillos blancos.")]),
    ]
    .spacing(6)
    .padding(18);
    shot(colors, column![bar, lines].into())
}

/// Página 4: código resaltado con la paleta de comandos encima.
fn coding(colors: &EditorColors) -> Element<'static, Message> {
    let c = colors.clone();
    let color = |kind| c.style(Some(kind)).color.unwrap_or(c.text);
    let keyword = color(HighlightKind::Keyword);
    let function = color(HighlightKind::Function);
    let string = color(HighlightKind::String);
    let comment = color(HighlightKind::Comment);
    let typ = color(HighlightKind::Type);
    let code_line = |number: &'static str, spans: Vec<iced::widget::text::Span<'static, ()>>| {
        row![
            ink(number, 12, c.line_number)
                .font(Font::MONOSPACE)
                .width(18),
            rich_text(spans)
                .size(12)
                .color(c.text)
                .font(Font::MONOSPACE),
        ]
        .spacing(10)
    };
    let code = column![
        code_line("1", vec![sp("// Romance sonámbulo").color(comment)]),
        code_line(
            "2",
            vec![
                sp("fn ").color(keyword),
                sp("verde").color(function),
                sp("() -> "),
                sp("String").color(typ),
                sp(" {")
            ]
        ),
        code_line(
            "3",
            vec![
                sp("    "),
                sp("let ").color(keyword),
                sp("viento = "),
                sp("\"verde\"").color(string),
                sp(";")
            ]
        ),
        code_line(
            "4",
            vec![
                sp("    "),
                sp("format!").color(function),
                sp("("),
                sp("\"{viento} ramas\"").color(string),
                sp(")")
            ]
        ),
        code_line("5", vec![sp("}")]),
        code_line("6", vec![]),
        code_line(
            "7",
            vec![
                sp("fn ").color(keyword),
                sp("barco").color(function),
                sp("() {}")
            ]
        ),
    ]
    .spacing(6)
    .padding(Padding::default().top(16).left(14));
    let entry = |label: &'static str, keys: &'static str, selected: bool| {
        container(
            row![
                ink(label, 12, c.text),
                Space::new().width(Length::Fill),
                ink(keys, 11, c.line_number),
            ]
            .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .width(Length::Fill)
        .style(highlight(if selected {
            c.selection
        } else {
            Color::TRANSPARENT
        }))
    };
    let palette = container(
        column![
            container(ink("> format", 12, c.text).font(Font::MONOSPACE))
                .padding([4, 8])
                .width(Length::Fill)
                .style(highlight(c.gutter)),
            entry("Formatear documento", keys::FORMAT, true),
            entry("Formatear JSON", "", false),
            entry("Formatear SQL", "", false),
        ]
        .spacing(2),
    )
    .padding(6)
    .width(Length::Fixed(270.0))
    .style({
        let (background, border) = (colors.background, colors.line_number);
        move |_: &Theme| {
            container::Style::default()
                .background(background)
                .border(Border {
                    color: border,
                    width: 1.0,
                    radius: 8.0.into(),
                })
                .shadow(Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                    offset: Vector::new(0.0, 6.0),
                    blur_radius: 18.0,
                })
        }
    });
    shot(
        colors,
        stack![
            container(code).width(Length::Fill).height(Length::Fill),
            container(palette).padding(Padding::default().top(122).left(110)),
        ]
        .into(),
    )
}

/// Página 5: pestañas recuperadas, zoom en la barra de estado y aviso de versión nueva.
fn memory(colors: &EditorColors) -> Element<'static, Message> {
    let c = colors.clone();
    let tab = |label: &'static str, active: bool| {
        container(ink(label, 12, c.text))
            .padding([5, 10])
            .style(highlight(if active {
                c.background
            } else {
                Color::TRANSPARENT
            }))
    };
    let tabs = strip(
        colors,
        row![
            tab("notas.md ●", true),
            tab("lista de la compra", false),
            tab("main.rs", false),
            ink("+", 13, c.line_number),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    );
    let page = column![
        ink("Pendiente para mañana", 18, c.text).font(Font {
            weight: iced::font::Weight::Bold,
            ..PROSE
        }),
        ink("•  Llamar a Federico", 14, c.text),
        ink("•  Ensayo de Yerma a las seis", 14, c.text),
        rich_text![sp("•  Recoger las fotos"), sp("|").color(c.cursor)]
            .size(14)
            .font(PROSE)
            .color(c.text),
    ]
    .spacing(6)
    .padding(18);
    let update = container(
        row![
            image(crate::update_dialog::icon()).width(26).height(26),
            column![
                ink("Lorca 0.2.0", 12, c.text).font(Font {
                    weight: iced::font::Weight::Semibold,
                    ..PROSE
                }),
                ink("ya está disponible", 11, c.line_number),
            ],
            Space::new().width(Length::Fill),
            container(ink("Actualizar", 11, c.background))
                .padding([3, 8])
                .style(highlight(c.cursor)),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(8)
    .width(Length::Fixed(240.0))
    .style({
        let (background, border) = (colors.background, colors.line_number);
        move |_: &Theme| {
            container::Style::default()
                .background(background)
                .border(Border {
                    color: border,
                    width: 1.0,
                    radius: 8.0.into(),
                })
                .shadow(Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 14.0,
                })
        }
    });
    let status = strip(
        colors,
        row![
            Space::new().width(Length::Fill),
            ink("Lín 4, Col 22", 11, c.text),
            container(ink("125 %", 11, c.text))
                .padding([1, 6])
                .style(highlight(c.selection)),
            ink("Texto con formato", 11, c.text),
            ink("UTF-8", 11, c.text),
        ]
        .spacing(12),
    );
    shot(
        colors,
        column![
            tabs,
            stack![
                page,
                container(update).padding(Padding::default().top(132).left(146)),
            ]
            .height(Length::Fill),
            status,
        ]
        .into(),
    )
}

/// Tramo de texto de una ilustración (sin vínculos).
fn sp(content: &'static str) -> iced::widget::text::Span<'static, ()> {
    span(content)
}

/// Fondo de un trozo de la ilustración.
fn highlight(color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_: &Theme| {
        container::Style::default()
            .background(color)
            .border(Border {
                radius: 4.0.into(),
                ..Border::default()
            })
    }
}

fn muted(theme: &Theme) -> text::Style {
    let palette = theme.extended_palette();
    text::Style {
        color: Some(Color {
            a: 0.7,
            ..palette.background.base.text
        }),
    }
}

fn accent(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme.extended_palette().primary.base.color),
    }
}
