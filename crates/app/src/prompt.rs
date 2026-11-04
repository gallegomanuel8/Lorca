//! Ventanas de confirmación propias («¿Guardar los cambios?», «Archivo cambiado en
//! disco»). Se dibujan sobre la interfaz con los colores del tema, en lugar de los
//! diálogos del sistema, que en Windows no admiten botones con texto propio.
//!
//! Mientras hay una abierta, `dialog_open` está activo (el editor no recibe teclas), Intro
//! pulsa el botón principal y Escape el de cancelar.

use iced::widget::{
    button, center, column, container, mouse_area, opaque, row, space, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Shadow, Theme, Vector};
use lorca_workspace::DocumentId;

use crate::app::{Message, Then};

/// Respuesta a «¿Guardar los cambios?».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveChoice {
    Save,
    Discard,
    Cancel,
}

/// Aspecto de un botón.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    /// Acción que pierde cambios.
    Danger,
}

#[derive(Debug, Clone)]
pub struct PromptButton {
    pub label: &'static str,
    pub kind: ButtonKind,
    pub message: Message,
}

/// Ventana de confirmación abierta.
#[derive(Debug, Clone)]
pub struct Prompt {
    pub title: String,
    pub body: String,
    /// De izquierda a derecha.
    pub buttons: Vec<PromptButton>,
    /// Botón que pulsa Intro.
    accept: usize,
    /// Botón que pulsa Escape.
    cancel: usize,
}

/// Anchura de la ventana.
const WIDTH: f32 = 440.0;

impl Prompt {
    /// «¿Guardar los cambios de `name`?» antes de cerrar el documento `id`.
    pub fn save_changes(name: &str, id: DocumentId, then: Then) -> Self {
        let answer = |choice| Message::SaveChoiceMade { id, choice, then };
        Self {
            title: "Cambios sin guardar".to_owned(),
            body: format!(
                "¿Quieres guardar los cambios de «{name}»? Si no los guardas, se perderán."
            ),
            buttons: vec![
                PromptButton {
                    label: "No guardar",
                    kind: ButtonKind::Danger,
                    message: answer(SaveChoice::Discard),
                },
                PromptButton {
                    label: "Cancelar",
                    kind: ButtonKind::Secondary,
                    message: answer(SaveChoice::Cancel),
                },
                PromptButton {
                    label: "Guardar",
                    kind: ButtonKind::Primary,
                    message: answer(SaveChoice::Save),
                },
            ],
            accept: 2,
            cancel: 1,
        }
    }

    /// Otro programa cambió `name` y hay cambios sin guardar: ¿recargar?
    pub fn external_change(name: &str, id: DocumentId) -> Self {
        let answer = |reload| Message::ExternalChangeChoice { id, reload };
        Self {
            title: "Archivo cambiado en disco".to_owned(),
            body: format!(
                "Otro programa ha modificado «{name}» y tienes cambios sin guardar. \
                 ¿Recargar el archivo del disco y perder tus cambios?"
            ),
            buttons: vec![
                PromptButton {
                    label: "Recargar",
                    kind: ButtonKind::Danger,
                    message: answer(true),
                },
                PromptButton {
                    label: "Conservar mis cambios",
                    kind: ButtonKind::Primary,
                    message: answer(false),
                },
            ],
            accept: 1,
            cancel: 1,
        }
    }

    /// Mensaje del botón principal (Intro).
    pub fn accept_message(&self) -> Option<Message> {
        self.buttons.get(self.accept).map(|b| b.message.clone())
    }

    /// Mensaje del botón de cancelar (Escape).
    pub fn cancel_message(&self) -> Option<Message> {
        self.buttons.get(self.cancel).map(|b| b.message.clone())
    }
}

/// Dibuja `prompt` centrado sobre `base`, con el fondo oscurecido y sin dejar pulsar
/// nada de debajo.
pub fn layer<'a>(base: Element<'a, Message>, prompt: &'a Prompt) -> Element<'a, Message> {
    let buttons = prompt
        .buttons
        .iter()
        .enumerate()
        .fold(row![].spacing(8), |buttons, (index, item)| {
            // El primer botón (el que pierde cambios) queda separado a la izquierda.
            let buttons = if index == 1 {
                buttons.push(space::horizontal())
            } else {
                buttons
            };
            buttons.push(prompt_button(item))
        })
        .align_y(Alignment::Center);

    let card = container(
        column![
            text(&prompt.title).size(16).font(Font {
                weight: iced::font::Weight::Semibold,
                ..Font::default()
            }),
            text(&prompt.body).size(13),
            space().height(Length::Fixed(6.0)),
            buttons,
        ]
        .spacing(10),
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
    // `opaque` y la zona de ratón vacía impiden que los clics lleguen a la interfaz.
    stack![base, opaque(mouse_area(backdrop))].into()
}

fn prompt_button(item: &PromptButton) -> Element<'_, Message> {
    let kind = item.kind;
    button(text(item.label).size(13))
        .padding([5, 14])
        .style(move |theme: &Theme, status| match kind {
            ButtonKind::Primary => button::primary(theme, status),
            ButtonKind::Secondary => button::secondary(theme, status),
            ButtonKind::Danger => {
                let mut style = button::secondary(theme, status);
                let danger = theme.extended_palette().danger;
                style.text_color = danger.base.color;
                if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                    style.background = Some(Background::Color(danger.weak.color));
                    style.text_color = danger.weak.text;
                }
                style
            }
        })
        .on_press(item.message.clone())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use lorca_workspace::Workspace;

    #[test]
    fn enter_saves_and_escape_cancels() {
        let mut workspace = Workspace::new();
        let id = workspace.new_document();
        let prompt = Prompt::save_changes("notas", id, Then::CloseTab);
        assert!(matches!(
            prompt.accept_message(),
            Some(Message::SaveChoiceMade {
                choice: SaveChoice::Save,
                ..
            })
        ));
        assert!(matches!(
            prompt.cancel_message(),
            Some(Message::SaveChoiceMade {
                choice: SaveChoice::Cancel,
                ..
            })
        ));
    }

    #[test]
    fn reload_needs_an_explicit_click() {
        let mut workspace = Workspace::new();
        let id = workspace.new_document();
        let prompt = Prompt::external_change("notas", id);
        // Intro y Escape conservan los cambios: recargar los perdería.
        for message in [prompt.accept_message(), prompt.cancel_message()] {
            assert!(matches!(
                message,
                Some(Message::ExternalChangeChoice { reload: false, .. })
            ));
        }
    }
}
