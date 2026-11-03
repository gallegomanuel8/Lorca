//! Menús contextuales (clic derecho) dibujados con iced.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::widget::{
    button, column, container, mouse_area, opaque, pin, row, rule, space, stack, text,
};
use iced::{Element, Event, Length, Point, Rectangle, Renderer, Shadow, Size, Theme, Vector};
use lorca_core::{LineEnding, TextEncoding};
use lorca_formats::Case;
use lorca_workspace::DocumentId;

use crate::app::Message;

const MENU_WIDTH: f32 = 250.0;
const ITEM_HEIGHT: f32 = 26.0;
const SEPARATOR_HEIGHT: f32 = 9.0;
const HEADER_HEIGHT: f32 = 22.0;
const MENU_PADDING: f32 = 5.0;

/// Sobre qué se abrió el menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTarget {
    Text(DocumentId),
    Tab(DocumentId),
    /// «UTF-8» de la barra de estado.
    Encoding(DocumentId),
    /// «LF/CRLF» de la barra de estado.
    LineEnding(DocumentId),
    /// «¶» de la barra de estado: caracteres invisibles y guías.
    View,
    /// Nombre del tema en la barra de estado.
    Theme,
    /// Menú de la barra de menús dibujada (Windows y Linux), por índice en [`crate::menu::BAR`].
    MenuBar(usize),
}

/// Menú abierto: posición (en la ventana) y objetivo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContextMenu {
    pub position: Point,
    pub target: MenuTarget,
}

/// Acciones de los menús contextuales.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextAction {
    Cut,
    Copy,
    Paste,
    Delete,
    SelectAll,
    Undo,
    Redo,
    FindSelection,
    FindAll,
    MarkAll,
    ToggleBookmark,
    GoToLine,
    Close(DocumentId),
    CloseOthers(DocumentId),
    CloseRight(DocumentId),
    CloseSelected,
    SaveSelected,
    CopyPath(DocumentId),
    Reveal(DocumentId),
    Split,
    Unsplit,
    SetLineEnding(LineEnding),
    SaveWithEncoding(TextEncoding),
    ReopenWithEncoding(TextEncoding),
    ToggleWhitespace,
    ToggleLineEndings,
    ToggleIndentGuides,
    ToggleWordWrap,
    FormatDocument,
    MinifyJson,
    ChangeCase(Case),
    TogglePreview,
    ExportPdf(DocumentId),
    SetThemeMode(lorca_workspace::ThemeMode),
    GoToDefinition,
    ShowHover,
    /// Índice del tema en el [`lorca_workspace::ThemeSet`].
    SetTheme(usize),
    /// Opción de la barra de menús (identificador de [`crate::menu::message_for`]).
    MenuCommand(&'static str),
}

/// Elemento de un menú.
#[derive(Debug, Clone, PartialEq)]
pub enum MenuItem {
    /// Texto, atajo y acción (`None` = deshabilitado).
    Action {
        label: String,
        shortcut: &'static str,
        action: Option<ContextAction>,
    },
    /// Título de una sección (no se puede pulsar).
    Header(String),
    Separator,
}

impl MenuItem {
    pub fn new(
        label: impl Into<String>,
        shortcut: &'static str,
        action: Option<ContextAction>,
    ) -> Self {
        Self::Action {
            label: label.into(),
            shortcut,
            action,
        }
    }

    /// Opción con marca de verificación delante si `checked`.
    pub fn check(label: &str, checked: bool, action: Option<ContextAction>) -> Self {
        let mark = if checked { "✓  " } else { "     " };
        Self::new(format!("{mark}{label}"), "", action)
    }
}

/// Altura que ocupará un menú con estos elementos.
pub fn menu_height(items: &[MenuItem]) -> f32 {
    let content: f32 = items
        .iter()
        .map(|item| match item {
            MenuItem::Action { .. } => ITEM_HEIGHT,
            MenuItem::Header(_) => HEADER_HEIGHT,
            MenuItem::Separator => SEPARATOR_HEIGHT,
        })
        .sum();
    content + MENU_PADDING * 2.0
}

/// Posición del menú ajustada para que no se salga de la ventana.
pub fn clamp_position(position: Point, height: f32, window: Size) -> Point {
    let x = position.x.min(window.width - MENU_WIDTH - 4.0).max(0.0);
    let y = if position.y + height > window.height {
        (position.y - height).max(0.0)
    } else {
        position.y
    };
    Point::new(x, y)
}

/// Capa con el menú sobre `base`. Un clic fuera del menú lo cierra.
pub fn layer<'a>(
    base: Element<'a, Message>,
    menu: &ContextMenu,
    items: Vec<MenuItem>,
    window: Size,
) -> Element<'a, Message> {
    let position = clamp_position(menu.position, menu_height(&items), window);
    let entries = items.into_iter().map(|item| match item {
        MenuItem::Separator => container(rule::horizontal(1)).padding([4, 6]).into(),
        MenuItem::Header(label) => container(text(label).size(11))
            .height(HEADER_HEIGHT)
            .padding([4, 10])
            .style(|theme: &Theme| {
                container::Style::default().color(theme.extended_palette().background.strong.color)
            })
            .into(),
        MenuItem::Action {
            label,
            shortcut,
            action,
        } => {
            let enabled = action.is_some();
            let content = row![
                text(label).size(13),
                space::horizontal(),
                text(shortcut).size(12).style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.color),
                }),
            ];
            button(content)
                .width(Length::Fill)
                .height(ITEM_HEIGHT)
                .padding([4, 10])
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let mut style = button::text(theme, status);
                    if !enabled {
                        style.text_color = palette.background.strong.color;
                    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                        style.background = Some(palette.primary.weak.color.into());
                        style.text_color = palette.primary.weak.text;
                    }
                    style
                })
                .on_press_maybe(action.map(Message::Context))
                .into()
        }
    });
    let menu = container(column(entries))
        .width(MENU_WIDTH)
        .padding(MENU_PADDING)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style::default()
                .background(palette.background.base.color)
                .border(iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                })
                .shadow(Shadow {
                    color: iced::Color {
                        a: 0.3,
                        ..iced::Color::BLACK
                    },
                    offset: Vector::new(0.0, 3.0),
                    blur_radius: 10.0,
                })
        });

    let backdrop = mouse_area(space().width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseContextMenu)
        .on_right_press(Message::CloseContextMenu);
    stack![base, opaque(stack![backdrop, pin(menu).position(position)])].into()
}

/// Envuelve `content` y publica `on_right_click(posición)` al hacer clic derecho sobre él.
pub fn context_area<'a>(
    content: impl Into<Element<'a, Message>>,
    on_right_click: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(ContextArea {
        content: content.into(),
        button: mouse::Button::Right,
        anchored: false,
        on_click: Box::new(on_right_click),
    })
}

/// Como [`context_area`], pero con el clic izquierdo (p. ej. para la barra de estado).
pub fn click_area<'a>(
    content: impl Into<Element<'a, Message>>,
    on_click: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(ContextArea {
        content: content.into(),
        button: mouse::Button::Left,
        anchored: false,
        on_click: Box::new(on_click),
    })
}

/// Como [`click_area`], pero publica la esquina inferior izquierda de `content` en vez de
/// la posición del clic (para abrir un menú justo debajo, como en la barra de menús).
pub fn anchor_area<'a>(
    content: impl Into<Element<'a, Message>>,
    on_click: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(ContextArea {
        content: content.into(),
        button: mouse::Button::Left,
        anchored: true,
        on_click: Box::new(on_click),
    })
}

struct ContextArea<'a> {
    content: Element<'a, Message>,
    button: mouse::Button,
    /// Publica la esquina inferior izquierda en vez de la posición del clic.
    anchored: bool,
    on_click: Box<dyn Fn(Point) -> Message + 'a>,
}

impl Widget<Message, Theme, Renderer> for ContextArea<'_> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        match tree.children.first_mut() {
            Some(child) => self.content.as_widget_mut().layout(child, renderer, limits),
            None => layout::Node::new(Size::ZERO),
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(child) = tree.children.first() {
            self.content
                .as_widget()
                .draw(child, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(child) = tree.children.first_mut() {
            self.content
                .as_widget_mut()
                .operate(child, layout, renderer, operation);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(child) = tree.children.first_mut() {
            self.content.as_widget_mut().update(
                child, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
        }
        if shell.is_event_captured() {
            return;
        }
        if let Event::Mouse(mouse::Event::ButtonPressed(button)) = event
            && *button == self.button
            && let Some(position) = cursor.position_over(layout.bounds())
        {
            let bounds = layout.bounds();
            let position = if self.anchored {
                Point::new(bounds.x, bounds.y + bounds.height)
            } else {
                position
            };
            shell.publish((self.on_click)(position));
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let inner = tree
            .children
            .first()
            .map_or(mouse::Interaction::default(), |child| {
                self.content
                    .as_widget()
                    .mouse_interaction(child, layout, cursor, viewport, renderer)
            });
        // Las zonas de clic izquierdo (barra de estado) muestran la mano al pasar.
        if self.button == mouse::Button::Left
            && inner == mouse::Interaction::default()
            && cursor.is_over(layout.bounds())
        {
            mouse::Interaction::Pointer
        } else {
            inner
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let child = tree.children.first_mut()?;
        self.content
            .as_widget_mut()
            .overlay(child, layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn height_counts_items_and_separators() {
        let items = vec![
            MenuItem::new("Copiar", "⌘C", Some(ContextAction::Copy)),
            MenuItem::Separator,
            MenuItem::new("Pegar", "⌘V", None),
        ];
        assert_eq!(
            menu_height(&items),
            ITEM_HEIGHT * 2.0 + SEPARATOR_HEIGHT + MENU_PADDING * 2.0
        );
    }

    #[test]
    fn menu_stays_inside_window() {
        let window = Size::new(800.0, 600.0);
        assert_eq!(
            clamp_position(Point::new(10.0, 10.0), 200.0, window),
            Point::new(10.0, 10.0)
        );
        // Demasiado a la derecha: se desplaza a la izquierda.
        assert_eq!(
            clamp_position(Point::new(790.0, 10.0), 200.0, window).x,
            800.0 - MENU_WIDTH - 4.0
        );
        // Demasiado abajo: se abre hacia arriba.
        assert_eq!(
            clamp_position(Point::new(10.0, 500.0), 200.0, window).y,
            300.0
        );
    }
}
