//! Atajos de teclado globales de la aplicación.
//!
//! Se escuchan todos los eventos de teclado, también los que ya ha capturado un control
//! (p. ej. el campo de búsqueda captura Escape). Por eso aquí solo se reconocen
//! combinaciones que ningún control usa para escribir.

use iced::event::{self, Event};
use iced::keyboard::{self, Key, Modifiers, key::Code, key::Named, key::Physical};
use iced::window;

use lorca_formats::Case;

use crate::app::{BookmarkAction, Message};
use crate::context_menu::ContextAction;

/// Traduce un evento en un mensaje de la aplicación (firma de `event::listen_with`).
pub fn shortcut(event: Event, _status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Message::MouseReleased)
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        }) => key_shortcut(&key, physical_key, modifiers),
        _ => None,
    }
}

/// Cmd en macOS y Ctrl en el resto (`Modifiers::command`), salvo Ctrl+Tab y Ctrl+G
/// (ir a línea), que usan Ctrl en todas las plataformas: en macOS ⌘G es «buscar siguiente».
fn key_shortcut(key: &Key, physical_key: Physical, modifiers: Modifiers) -> Option<Message> {
    let is_mac = cfg!(target_os = "macos");
    let shift = modifiers.shift();

    if let Key::Named(named) = key {
        return match named {
            Named::Escape => Some(Message::Escape),
            Named::Tab if modifiers.control() => Some(if shift {
                Message::PreviousTab
            } else {
                Message::NextTab
            }),
            Named::F3 if modifiers.command() => Some(Message::FindWordUnderCursor),
            Named::F3 if shift => Some(Message::FindPrevious),
            Named::F3 => Some(Message::FindNext),
            Named::F2 if modifiers.command() => Some(Message::Bookmark(BookmarkAction::Toggle)),
            Named::F2 if shift => Some(Message::Bookmark(BookmarkAction::Previous)),
            Named::F2 => Some(Message::Bookmark(BookmarkAction::Next)),
            _ => None,
        };
    }

    // ⌘\ divide la pantalla. Se reconoce por la tecla física para que funcione también en
    // teclados donde esa tecla tiene otro carácter (en español es «ç»).
    if modifiers.command() && !modifiers.alt() && physical_key == Physical::Code(Code::Backslash) {
        return Some(Message::ToggleSplit);
    }
    // Tamaño de letra. «+» se acepta con o sin Mayús (en inglés está en la tecla «=»).
    if modifiers.command()
        && !modifiers.alt()
        && let Key::Character(c) = key
    {
        match c.as_str() {
            "+" | "=" => return Some(Message::Zoom(1)),
            "-" => return Some(Message::Zoom(-1)),
            "0" => return Some(Message::Zoom(0)),
            "," => return Some(Message::OpenConfig),
            _ => {}
        }
    }
    let letter = key.to_latin(physical_key)?.to_ascii_lowercase();
    if letter == 'g' && modifiers.control() && !modifiers.alt() {
        return Some(Message::GoToLine);
    }
    if !modifiers.command() {
        return None;
    }
    // ⌘H oculta la aplicación en macOS: allí sustituir es ⌥⌘F, como en Xcode o VS Code.
    if modifiers.alt() {
        return match letter {
            'f' if is_mac => Some(Message::OpenReplace),
            'l' => Some(Message::Context(ContextAction::FormatDocument)),
            _ => None,
        };
    }
    match letter {
        'n' if !shift => Some(Message::NewDocument),
        'o' if !shift => Some(Message::Open),
        's' if shift => Some(Message::SaveAs),
        's' => Some(Message::Save),
        'w' if !shift => Some(Message::CloseActive),
        'z' if shift => Some(Message::Redo),
        'z' => Some(Message::Undo),
        'y' if !is_mac => Some(Message::Redo),
        'f' if shift => Some(Message::FindInFolder),
        'f' => Some(Message::OpenFind),
        'p' if shift => Some(Message::OpenPalette),
        'h' if !is_mac && !shift => Some(Message::OpenReplace),
        '\\' => Some(Message::ToggleSplit),
        'u' if shift => Some(Message::Context(ContextAction::ChangeCase(Case::Upper))),
        'u' => Some(Message::Context(ContextAction::ChangeCase(Case::Lower))),
        'v' if shift => Some(Message::Context(ContextAction::TogglePreview)),
        'g' if is_mac && shift => Some(Message::FindPrevious),
        'g' if is_mac => Some(Message::FindNext),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn char_key(c: &str, code: Code, modifiers: Modifiers) -> Option<Message> {
        key_shortcut(&Key::Character(c.into()), Physical::Code(code), modifiers)
    }

    fn named(n: Named, modifiers: Modifiers) -> Option<Message> {
        key_shortcut(&Key::Named(n), Physical::Code(Code::F1), modifiers)
    }

    #[test]
    fn file_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("n", Code::KeyN, cmd),
            Some(Message::NewDocument)
        ));
        assert!(matches!(
            char_key("o", Code::KeyO, cmd),
            Some(Message::Open)
        ));
        assert!(matches!(
            char_key("s", Code::KeyS, cmd),
            Some(Message::Save)
        ));
        assert!(matches!(
            char_key("S", Code::KeyS, cmd | Modifiers::SHIFT),
            Some(Message::SaveAs)
        ));
        assert!(matches!(
            char_key("w", Code::KeyW, cmd),
            Some(Message::CloseActive)
        ));
    }

    #[test]
    fn zoom() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("+", Code::BracketRight, cmd),
            Some(Message::Zoom(1))
        ));
        assert!(matches!(
            char_key("=", Code::Equal, cmd),
            Some(Message::Zoom(1))
        ));
        assert!(matches!(
            char_key("-", Code::Slash, cmd),
            Some(Message::Zoom(-1))
        ));
        assert!(matches!(
            char_key("0", Code::Digit0, cmd),
            Some(Message::Zoom(0))
        ));
        assert!(char_key("+", Code::BracketRight, Modifiers::empty()).is_none());
    }

    #[test]
    fn undo_redo() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("z", Code::KeyZ, cmd),
            Some(Message::Undo)
        ));
        assert!(matches!(
            char_key("Z", Code::KeyZ, cmd | Modifiers::SHIFT),
            Some(Message::Redo)
        ));
    }

    #[test]
    fn tab_cycling_uses_ctrl_everywhere() {
        assert!(matches!(
            named(Named::Tab, Modifiers::CTRL),
            Some(Message::NextTab)
        ));
        assert!(matches!(
            named(Named::Tab, Modifiers::CTRL | Modifiers::SHIFT),
            Some(Message::PreviousTab)
        ));
    }

    #[test]
    fn search_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("f", Code::KeyF, cmd),
            Some(Message::OpenFind)
        ));
        assert!(matches!(
            named(Named::F3, Modifiers::empty()),
            Some(Message::FindNext)
        ));
        assert!(matches!(
            named(Named::F3, Modifiers::SHIFT),
            Some(Message::FindPrevious)
        ));
        assert!(matches!(
            named(Named::F3, cmd),
            Some(Message::FindWordUnderCursor)
        ));
        assert!(matches!(
            named(Named::Escape, Modifiers::empty()),
            Some(Message::Escape)
        ));
        assert!(matches!(
            char_key("g", Code::KeyG, Modifiers::CTRL),
            Some(Message::GoToLine)
        ));
        let replace = if cfg!(target_os = "macos") {
            char_key("f", Code::KeyF, cmd | Modifiers::ALT)
        } else {
            char_key("h", Code::KeyH, cmd)
        };
        assert!(matches!(replace, Some(Message::OpenReplace)));
    }

    #[test]
    fn split_shortcut_by_physical_key() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("ç", Code::Backslash, cmd),
            Some(Message::ToggleSplit)
        ));
        assert!(matches!(
            char_key("\\", Code::Backslash, cmd),
            Some(Message::ToggleSplit)
        ));
    }

    #[test]
    fn format_and_case_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("l", Code::KeyL, cmd | Modifiers::ALT),
            Some(Message::Context(ContextAction::FormatDocument))
        ));
        assert!(matches!(
            char_key("U", Code::KeyU, cmd | Modifiers::SHIFT),
            Some(Message::Context(ContextAction::ChangeCase(Case::Upper)))
        ));
        assert!(matches!(
            char_key("u", Code::KeyU, cmd),
            Some(Message::Context(ContextAction::ChangeCase(Case::Lower)))
        ));
        assert!(matches!(
            char_key("V", Code::KeyV, cmd | Modifiers::SHIFT),
            Some(Message::Context(ContextAction::TogglePreview))
        ));
    }

    #[test]
    fn bookmark_shortcuts() {
        assert!(matches!(
            named(Named::F2, Modifiers::COMMAND),
            Some(Message::Bookmark(BookmarkAction::Toggle))
        ));
        assert!(matches!(
            named(Named::F2, Modifiers::empty()),
            Some(Message::Bookmark(BookmarkAction::Next))
        ));
        assert!(matches!(
            named(Named::F2, Modifiers::SHIFT),
            Some(Message::Bookmark(BookmarkAction::Previous))
        ));
    }

    #[test]
    fn plain_keys_are_ignored() {
        assert!(char_key("s", Code::KeyS, Modifiers::empty()).is_none());
        assert!(char_key("s", Code::KeyS, Modifiers::COMMAND | Modifiers::ALT).is_none());
    }
}
