//! Atajos de teclado globales de la aplicación.

use iced::keyboard::{self, Key, key::Named};

use crate::app::Message;

/// Traduce una tecla no capturada por el editor en un mensaje de la aplicación.
///
/// Cmd en macOS y Ctrl en el resto (`Modifiers::command`), salvo Ctrl+Tab, que es Ctrl en
/// todas las plataformas.
pub fn shortcut(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        ..
    } = event
    else {
        return None;
    };
    let shift = modifiers.shift();

    if key == Key::Named(Named::Tab) && modifiers.control() {
        return Some(if shift {
            Message::PreviousTab
        } else {
            Message::NextTab
        });
    }
    if !modifiers.command() || modifiers.alt() {
        return None;
    }
    match key.to_latin(physical_key)?.to_ascii_lowercase() {
        'n' if !shift => Some(Message::NewDocument),
        'o' if !shift => Some(Message::Open),
        's' if shift => Some(Message::SaveAs),
        's' => Some(Message::Save),
        'w' if !shift => Some(Message::CloseActive),
        'z' if shift => Some(Message::Redo),
        'z' => Some(Message::Undo),
        'y' if !cfg!(target_os = "macos") => Some(Message::Redo),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::{Code, Physical};
    use iced::keyboard::{Location, Modifiers};

    fn press(key: Key, code: Code, modifiers: Modifiers) -> Option<Message> {
        shortcut(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: Physical::Code(code),
            location: Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    fn char_key(c: &str, code: Code, modifiers: Modifiers) -> Option<Message> {
        press(Key::Character(c.into()), code, modifiers)
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
        let tab = || Key::Named(Named::Tab);
        assert!(matches!(
            press(tab(), Code::Tab, Modifiers::CTRL),
            Some(Message::NextTab)
        ));
        assert!(matches!(
            press(tab(), Code::Tab, Modifiers::CTRL | Modifiers::SHIFT),
            Some(Message::PreviousTab)
        ));
    }

    #[test]
    fn plain_keys_are_ignored() {
        assert!(char_key("s", Code::KeyS, Modifiers::empty()).is_none());
        assert!(char_key("s", Code::KeyS, Modifiers::COMMAND | Modifiers::ALT).is_none());
    }
}
