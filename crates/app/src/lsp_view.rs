//! Suscripción que trae los eventos de los servidores de lenguaje y desplegables de
//! autocompletado e información.

use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, Stream, StreamExt};
use iced::widget::{button, column, container, markdown, pin, row, scrollable, space, stack, text};
use iced::{Alignment, Element, Font, Length, Point, Shadow, Size, Subscription, Theme, Vector};
use lorca_lsp::CompletionItem;

use crate::app::Message;

const COMPLETION_WIDTH: f32 = 440.0;
const ROW_HEIGHT: f32 = 24.0;
const HOVER_WIDTH: f32 = 520.0;
const HOVER_HEIGHT: f32 = 280.0;

/// Eventos de todos los servidores. Al arrancar envía `LspConnected` con el canal por el
/// que los clientes mandan sus eventos.
pub fn events() -> Subscription<Message> {
    Subscription::run(stream)
}

fn stream() -> impl Stream<Item = Message> {
    iced::stream::channel(64, async move |mut output| {
        let (sender, mut receiver) = mpsc::unbounded();
        if output.send(Message::LspConnected(sender)).await.is_err() {
            return;
        }
        while let Some(event) = receiver.next().await {
            if output.send(Message::Lsp(event)).await.is_err() {
                break;
            }
        }
    })
}

fn popup_style(theme: &Theme) -> container::Style {
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
}

/// Coloca un desplegable bajo `anchor` (o encima si no cabe), dentro de la ventana.
fn place(anchor: Point, size: Size, window: Size, line_height: f32) -> Point {
    let x = anchor.x.min(window.width - size.width - 8.0).max(4.0);
    let below = anchor.y + 2.0;
    let y = if below + size.height > window.height - 24.0 {
        (anchor.y - line_height - size.height - 2.0).max(4.0)
    } else {
        below
    };
    Point::new(x, y)
}

/// Lista de propuestas sobre `base`.
pub fn completion_layer<'a>(
    base: Element<'a, Message>,
    rows: Vec<(usize, &'a CompletionItem)>,
    selected: usize,
    anchor: Point,
    window: Size,
    line_height: f32,
) -> Element<'a, Message> {
    let mut list = column![];
    for (row_index, item) in rows.iter().copied() {
        let is_selected = row_index == selected;
        let detail = item.detail.clone().unwrap_or_default();
        let content = row![
            container(text(item.kind).size(10).font(Font::MONOSPACE))
                .width(38)
                .align_x(Alignment::Center),
            text(item.label.clone())
                .size(13)
                .font(Font::MONOSPACE)
                .wrapping(text::Wrapping::None),
            space::horizontal(),
            text(detail)
                .size(11)
                .wrapping(text::Wrapping::None)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.color),
                }),
        ]
        .spacing(6)
        .align_y(Alignment::Center);
        list = list.push(
            button(content)
                .width(Length::Fill)
                .height(ROW_HEIGHT)
                .padding([2, 6])
                .clip(true)
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let mut style = button::text(theme, status);
                    if is_selected || matches!(status, button::Status::Hovered) {
                        style.background = Some(palette.primary.weak.color.into());
                        style.text_color = palette.primary.weak.text;
                    }
                    style
                })
                .on_press(Message::CompletionPick(row_index)),
        );
    }
    let height = rows.len() as f32 * ROW_HEIGHT + 8.0;
    let size = Size::new(COMPLETION_WIDTH, height);
    let panel = container(list)
        .width(COMPLETION_WIDTH)
        .padding(4)
        .clip(true)
        .style(popup_style);
    let position = place(anchor, size, window, line_height);
    stack![base, pin(panel).position(position)].into()
}

/// Información (hover) en Markdown sobre `base`.
pub fn hover_layer<'a>(
    base: Element<'a, Message>,
    content: &'a markdown::Content,
    theme: &Theme,
    anchor: Point,
    window: Size,
    line_height: f32,
) -> Element<'a, Message> {
    let settings = markdown::Settings::with_text_size(13, theme.clone());
    let rendered = markdown::view(content.items(), settings).map(Message::OpenUrl);
    let panel = container(
        scrollable(container(rendered).padding([8, 12]).width(Length::Fill)).height(Length::Shrink),
    )
    .width(HOVER_WIDTH)
    .max_height(HOVER_HEIGHT)
    .style(popup_style);
    let position = place(
        anchor,
        Size::new(HOVER_WIDTH, HOVER_HEIGHT),
        window,
        line_height,
    );
    stack![base, pin(panel).position(position)].into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popups_stay_inside_the_window() {
        let window = Size::new(800.0, 600.0);
        let size = Size::new(300.0, 200.0);
        // Cabe debajo.
        assert_eq!(
            place(Point::new(100.0, 100.0), size, window, 20.0),
            Point::new(100.0, 102.0)
        );
        // Abajo del todo: encima de la línea.
        let up = place(Point::new(100.0, 560.0), size, window, 20.0);
        assert_eq!(up.y, 560.0 - 20.0 - 200.0 - 2.0);
        // Pegado a la derecha: se desplaza a la izquierda.
        assert_eq!(place(Point::new(700.0, 100.0), size, window, 20.0).x, 492.0);
    }
}
