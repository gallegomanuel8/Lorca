//! Barra de formato con las opciones del Bloc de notas de Windows 11: encabezado, listas
//! (con sangría), negrita, cursiva, tachado, vínculo, tabla y borrar formato. Los
//! desplegables (encabezado, listas y tabla) se dibujan encima de la ventana bajo su
//! botón; un clic fuera los cierra.
//!
//! Solo actúa en documentos de texto y Markdown (`BarState::enabled`).

use iced::font::{Style, Weight};
use iced::widget::{
    button, column, container, mouse_area, opaque, pin, rich_text, row, rule, space, span, stack,
    svg, text, tooltip,
};
use iced::{Alignment, Border, Color, Element, Font, Length, Point, Shadow, Size, Theme, Vector};
use lorca_formats::{Inline, ListKind, TableEdit};

use crate::app::Message;
use crate::context_menu::{ContextAction, anchor_area};
use crate::shortcut_labels as keys;

/// Desplegables de la barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatMenu {
    Heading,
    List,
    Table,
}

/// Desplegable abierto: cuál, dónde y, en el de tabla, la celda bajo el ratón.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FormatPopup {
    pub menu: FormatMenu,
    pub position: Point,
    /// Fila y columna (desde 0) de la cuadrícula de «Insertar tabla» bajo el ratón.
    pub table_hover: Option<(usize, usize)>,
}

/// Lo que la barra necesita saber del documento activo.
#[derive(Debug, Clone, Copy, Default)]
pub struct BarState {
    /// El documento es de texto o Markdown.
    pub enabled: bool,
    /// Encabezado de la línea del cursor (`None`: texto normal).
    pub heading: Option<u8>,
    /// El cursor está en un elemento de lista (se puede cambiar la sangría).
    pub in_list: bool,
    /// El cursor está en una tabla (se puede modificar).
    pub in_table: bool,
}

/// Tamaño de la cuadrícula de «Insertar tabla».
const GRID: usize = 5;
/// Opciones del desplegable de encabezado: nombre, nivel y tamaño de letra de muestra.
const HEADINGS: [(&str, Option<u8>, f32); 7] = [
    ("Título", Some(1), 26.0),
    ("Subtítulo", Some(2), 22.0),
    ("Encabezado", Some(3), 19.0),
    ("Subtítulo", Some(4), 16.0),
    ("Sección", Some(5), 14.0),
    ("Subsección", Some(6), 13.0),
    ("Cuerpo", None, 12.0),
];

const CHEVRON: &[u8] = include_bytes!("../icons/chevron.svg");
const LIST: &[u8] = include_bytes!("../icons/list.svg");
const LINK: &[u8] = include_bytes!("../icons/link.svg");
const TABLE: &[u8] = include_bytes!("../icons/table.svg");
const IMAGE: &[u8] = include_bytes!("../icons/image.svg");
const CLEAR: &[u8] = include_bytes!("../icons/clear-format.svg");

/// Icono SVG teñido con el color del texto (o atenuado si está desactivado).
fn icon(bytes: &'static [u8], size: f32, enabled: bool) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(bytes))
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |theme: &Theme, _status| {
            let palette = theme.extended_palette();
            svg::Style {
                color: Some(if enabled {
                    palette.background.base.text
                } else {
                    palette.background.strong.color
                }),
            }
        })
        .into()
}

fn tip<'a>(content: impl Into<Element<'a, Message>>, label: String) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(label).size(12))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .into()
}

fn with_keys(name: &str, shortcut: &str) -> String {
    if shortcut.is_empty() {
        name.to_owned()
    } else {
        format!("{name} ({shortcut})")
    }
}

/// Estilo de los botones de la barra: transparentes, con fondo al pasar el ratón.
fn tool_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::text(theme, status);
    style.text_color = palette.background.base.text;
    style.border = Border {
        radius: 4.0.into(),
        ..Border::default()
    };
    match status {
        button::Status::Hovered | button::Status::Pressed => {
            style.background = Some(palette.background.weak.color.into());
        }
        button::Status::Disabled => style.text_color = palette.background.strong.color,
        button::Status::Active => {}
    }
    style
}

/// Botón que aplica `action` al pulsarlo.
fn tool<'a>(
    content: impl Into<Element<'a, Message>>,
    label: String,
    action: ContextAction,
    enabled: bool,
) -> Element<'a, Message> {
    tip(
        button(content)
            .padding([4, 7])
            .style(tool_style)
            .on_press_maybe(enabled.then_some(Message::Context(action))),
        label,
    )
}

/// Botón que abre un desplegable bajo él.
fn menu_tool<'a>(
    content: Element<'a, Message>,
    label: &str,
    menu: FormatMenu,
    enabled: bool,
) -> Element<'a, Message> {
    let content = row![content, icon(CHEVRON, 12.0, enabled)]
        .spacing(3)
        .align_y(Alignment::Center);
    // Contenedor y no botón: `anchor_area` necesita recibir el clic para saber dónde
    // abrir el desplegable.
    let area = container(content).padding([4, 7]);
    let area: Element<'a, Message> = if enabled {
        anchor_area(
            mouse_area(area).interaction(iced::mouse::Interaction::Pointer),
            move |position| Message::OpenFormatMenu(menu, position),
        )
    } else {
        area.into()
    };
    tip(area, label.to_owned())
}

fn separator<'a>() -> Element<'a, Message> {
    container(
        space()
            .width(Length::Fixed(1.0))
            .height(Length::Fixed(18.0)),
    )
    .style(|theme: &Theme| {
        container::Style::default().background(theme.extended_palette().background.strong.color)
    })
    .into()
}

/// La barra. `background` es el color de la barra de pestañas, que continúa debajo.
pub fn bar<'a>(state: BarState, background: fn(&Theme) -> Color) -> Element<'a, Message> {
    let enabled = state.enabled;
    let font = |weight, style| Font {
        weight,
        style,
        ..Font::default()
    };
    let heading_label = match state.heading {
        Some(level) => format!("H{level}"),
        None => "¶".to_owned(),
    };
    let tools = row![
        menu_tool(
            text(heading_label)
                .size(13)
                .font(font(Weight::Semibold, Style::Normal))
                .into(),
            "Encabezado",
            FormatMenu::Heading,
            enabled,
        ),
        menu_tool(
            icon(LIST, 16.0, enabled),
            "Listas",
            FormatMenu::List,
            enabled
        ),
        separator(),
        tool(
            text("B").size(15).font(font(Weight::Bold, Style::Normal)),
            with_keys("Negrita", keys::BOLD),
            ContextAction::Inline(Inline::Bold),
            enabled,
        ),
        tool(
            text("I").size(15).font(font(Weight::Normal, Style::Italic)),
            with_keys("Cursiva", keys::ITALIC),
            ContextAction::Inline(Inline::Italic),
            enabled,
        ),
        tool(
            rich_text![span::<(), _>("S").strikethrough(true)].size(15),
            "Tachado".to_owned(),
            ContextAction::Inline(Inline::Strikethrough),
            enabled,
        ),
        tool(
            icon(LINK, 16.0, enabled),
            with_keys("Vínculo", keys::LINK),
            ContextAction::Link,
            enabled,
        ),
        tool(
            icon(IMAGE, 16.0, enabled),
            "Insertar imagen (también se pueden pegar o arrastrar)".to_owned(),
            ContextAction::InsertImage,
            enabled,
        ),
        menu_tool(
            icon(TABLE, 16.0, enabled),
            "Tabla",
            FormatMenu::Table,
            enabled
        ),
        separator(),
        tool(
            icon(CLEAR, 16.0, enabled),
            "Borrar formato".to_owned(),
            ContextAction::ClearFormatting,
            enabled,
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let mut content = row![tools].align_y(Alignment::Center);
    if !enabled {
        content = content.push(space().width(Length::Fixed(10.0))).push(
            text("El formato es para texto y Markdown")
                .size(12)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.color),
                }),
        );
    }
    container(content)
        .padding([3, 8])
        .width(Length::Fill)
        .style(move |theme: &Theme| container::Style::default().background(background(theme)))
        .into()
}

/// Opción de un desplegable. `selected` pone la marca de color a la izquierda.
fn entry<'a>(
    content: impl Into<Element<'a, Message>> + 'a,
    action: Option<ContextAction>,
    selected: bool,
) -> Element<'a, Message> {
    let enabled = action.is_some();
    let mark = container(
        space()
            .width(Length::Fixed(3.0))
            .height(Length::Fixed(16.0)),
    )
    .style(move |theme: &Theme| {
        let color = if selected {
            theme.extended_palette().primary.base.color
        } else {
            Color::TRANSPARENT
        };
        container::Style::default()
            .background(color)
            .border(Border {
                radius: 2.0.into(),
                ..Border::default()
            })
    });
    let content: Element<'a, Message> = content.into();
    button(row![mark, content].spacing(9).align_y(Alignment::Center))
        .width(Length::Fill)
        .padding([6, 6])
        .style(move |theme: &Theme, status| {
            let palette = theme.extended_palette();
            let mut style = tool_style(theme, status);
            if !enabled {
                style.text_color = palette.background.strong.color;
            } else if selected && status == button::Status::Active {
                style.background = Some(palette.background.weak.color.into());
            }
            style
        })
        .on_press_maybe(action.map(Message::Context))
        .into()
}

fn heading_menu<'a>(state: BarState) -> Element<'a, Message> {
    let entries = HEADINGS.iter().map(|&(name, level, size)| {
        let label = text(name).size(size).font(Font {
            weight: if level.is_some() {
                Weight::Semibold
            } else {
                Weight::Normal
            },
            ..Font::default()
        });
        entry(
            label,
            Some(ContextAction::Heading(level)),
            state.heading == level,
        )
    });
    column(entries).width(Length::Fixed(210.0)).into()
}

fn list_menu<'a>(state: BarState) -> Element<'a, Message> {
    let label = |content: &'static str| text(content).size(13);
    column![
        entry(
            label("Lista con viñetas"),
            Some(ContextAction::List(ListKind::Bullet)),
            false
        ),
        entry(
            label("Lista numerada"),
            Some(ContextAction::List(ListKind::Numbered)),
            false
        ),
        entry(
            label("Aumentar sangría"),
            state.in_list.then_some(ContextAction::IndentList(true)),
            false
        ),
        entry(
            label("Reducir sangría"),
            state.in_list.then_some(ContextAction::IndentList(false)),
            false
        ),
    ]
    .width(Length::Fixed(210.0))
    .into()
}

fn table_menu<'a>(state: BarState, hover: Option<(usize, usize)>) -> Element<'a, Message> {
    const CELL: f32 = 26.0;
    let grid = column((0..GRID).map(|row_index| {
        row((0..GRID).map(|column_index| {
            let lit = hover.is_some_and(|(r, c)| row_index <= r && column_index <= c);
            let square = container(
                space()
                    .width(Length::Fixed(CELL))
                    .height(Length::Fixed(CELL)),
            )
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style::default()
                    .background(if lit {
                        palette.primary.weak.color
                    } else {
                        palette.background.weak.color
                    })
                    .border(Border {
                        color: if lit {
                            palette.primary.base.color
                        } else {
                            palette.background.strong.color
                        },
                        width: 1.0,
                        radius: 4.0.into(),
                    })
            });
            mouse_area(square)
                .on_enter(Message::TableHover(Some((row_index, column_index))))
                .on_press(Message::Context(ContextAction::InsertTable {
                    rows: row_index + 1,
                    columns: column_index + 1,
                }))
                .interaction(iced::mouse::Interaction::Pointer)
                .into()
        }))
        .spacing(5)
        .into()
    }))
    .spacing(5);
    let title = match hover {
        Some((rows, columns)) => format!("Insertar tabla: {} × {}", columns + 1, rows + 1),
        None => "Insertar tabla".to_owned(),
    };
    let label = |content: &'static str| text(content).size(13);
    let edit = |content, edit| {
        entry(
            label(content),
            state.in_table.then_some(ContextAction::Table(edit)),
            false,
        )
    };
    column![
        container(text(title).size(12)).padding([2, 6]),
        container(mouse_area(grid).on_exit(Message::TableHover(None))).padding([4, 6]),
        container(rule::horizontal(1)).padding([4, 2]),
        entry(
            label("Insertar tabla"),
            Some(ContextAction::InsertTable {
                rows: 3,
                columns: 3,
            }),
            false
        ),
        container(text("Modificar tabla").size(12)).padding([6, 6]),
        edit("Insertar fila debajo", TableEdit::InsertRow),
        edit("Insertar columna a la derecha", TableEdit::InsertColumn),
        edit("Eliminar fila", TableEdit::DeleteRow),
        edit("Eliminar columna", TableEdit::DeleteColumn),
    ]
    .width(Length::Fixed(230.0))
    .into()
}

/// Dibuja el desplegable abierto sobre `base`.
pub fn layer<'a>(
    base: Element<'a, Message>,
    popup: FormatPopup,
    state: BarState,
    window: Size,
) -> Element<'a, Message> {
    let (content, width) = match popup.menu {
        FormatMenu::Heading => (heading_menu(state), 210.0),
        FormatMenu::List => (list_menu(state), 210.0),
        FormatMenu::Table => (table_menu(state, popup.table_hover), 230.0),
    };
    let menu = container(content).padding(6).style(|theme: &Theme| {
        let palette = theme.extended_palette();
        container::Style::default()
            .background(palette.background.base.color)
            .color(palette.background.base.text)
            .border(Border {
                color: palette.background.strong.color,
                width: 1.0,
                radius: 8.0.into(),
            })
            .shadow(Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.3),
                offset: Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            })
    });
    let x = popup.position.x.min(window.width - width - 20.0).max(0.0);
    let position = Point::new(x, popup.position.y + 2.0);
    let backdrop = mouse_area(space().width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseFormatMenu)
        .on_right_press(Message::CloseFormatMenu);
    stack![base, opaque(stack![backdrop, pin(menu).position(position)])].into()
}
