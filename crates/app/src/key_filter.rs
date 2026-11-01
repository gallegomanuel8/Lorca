//! Envoltorio que impide que los atajos con ⌘ escriban en un campo de texto (macOS).
//!
//! El `text_input` de iced inserta el texto de cualquier pulsación aunque esté pulsado ⌘:
//! ⌘G escribiría «g» y ⌥⌘F escribiría «ƒ». En Windows y Linux no hace falta: Ctrl+letra
//! produce caracteres de control (que se descartan) y Ctrl+Alt es AltGr, que sí escribe.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Operation, Tree, Widget};
use iced::advanced::{Clipboard, Shell, mouse, overlay, renderer};
use iced::keyboard::{self, Key};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

/// Envuelve `content` filtrando las pulsaciones con ⌘ (salvo ⌘A, ⌘C, ⌘V y ⌘X).
pub fn filter_command_keys<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    Element::new(KeyFilter {
        content: content.into(),
    })
}

struct KeyFilter<'a, Message> {
    content: Element<'a, Message>,
}

/// `true` si la pulsación debe ocultarse al contenido.
fn is_blocked(event: &Event) -> bool {
    let Event::Keyboard(keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    }) = event
    else {
        return false;
    };
    if !cfg!(target_os = "macos") || !modifiers.logo() {
        return false;
    }
    let editing = matches!(key.to_latin(*physical_key), Some('a' | 'c' | 'v' | 'x'));
    !editing && !matches!(key, Key::Named(_))
}

impl<Message> Widget<Message, Theme, Renderer> for KeyFilter<'_, Message> {
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
        if is_blocked(event) {
            return;
        }
        if let Some(child) = tree.children.first_mut() {
            self.content.as_widget_mut().update(
                child, event, layout, cursor, renderer, clipboard, shell, viewport,
            );
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
        tree.children
            .first()
            .map_or(mouse::Interaction::default(), |child| {
                self.content
                    .as_widget()
                    .mouse_interaction(child, layout, cursor, viewport, renderer)
            })
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
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Location, Modifiers};

    fn press(c: &str, code: Code, modifiers: Modifiers) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Character(c.into()),
            modified_key: Key::Character(c.into()),
            physical_key: Physical::Code(code),
            location: Location::Standard,
            modifiers,
            text: Some(c.into()),
            repeat: false,
        })
    }

    #[test]
    fn plain_typing_passes() {
        assert!(!is_blocked(&press("g", Code::KeyG, Modifiers::empty())));
        assert!(!is_blocked(&press(
            "@",
            Code::Digit2,
            Modifiers::CTRL | Modifiers::ALT
        )));
    }

    #[test]
    fn command_shortcuts_are_blocked_only_on_macos() {
        let blocked = is_blocked(&press("g", Code::KeyG, Modifiers::LOGO));
        assert_eq!(blocked, cfg!(target_os = "macos"));
        assert!(!is_blocked(&press("v", Code::KeyV, Modifiers::LOGO)));
    }
}
