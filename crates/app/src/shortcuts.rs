//! Atajos de teclado globales de la aplicación.
//!
//! Se escuchan todos los eventos de teclado, también los que ya ha capturado un control
//! (p. ej. el campo de búsqueda captura Escape). Por eso aquí solo se reconocen
//! combinaciones que ningún control usa para escribir.

use iced::event::{self, Event};
use iced::keyboard::{self, Key, Modifiers, key::Code, key::Named, key::Physical};
use iced::window;

use lorca_formats::{Case, Inline, ListKind};

use crate::app::{BookmarkAction, Message};
use crate::context_menu::ContextAction;

/// Traduce un evento en un mensaje de la aplicación (firma de `event::listen_with`).
pub fn shortcut(event: Event, status: event::Status, _window: window::Id) -> Option<Message> {
    match event {
        // ←/→ sin capturar (no hay editor a la vista): páginas del resumen de funciones.
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(named @ (Named::ArrowLeft | Named::ArrowRight)),
            modifiers,
            ..
        }) if status == event::Status::Ignored && modifiers.is_empty() => {
            Some(Message::TourNav(if named == Named::ArrowRight {
                1
            } else {
                -1
            }))
        }
        Event::Window(window::Event::FileDropped(path)) => Some(Message::FileDropped(path)),
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Some(Message::ModifiersChanged(modifiers))
        }
        // Soltar el botón termina el arrastre de una pestaña. Si se suelta fuera de la
        // ventana no llega el evento: salir de ella también lo termina (si no, pasar luego
        // el ratón por las pestañas las seguiría moviendo).
        Event::Mouse(
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)
            | iced::mouse::Event::CursorLeft,
        ) => Some(Message::MouseReleased),
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
            // Solo hace algo con una ventana de confirmación abierta.
            Named::Enter if !modifiers.command() && !modifiers.alt() => Some(Message::PromptAccept),
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
            Named::F8 => Some(Message::NextProblem(!shift)),
            Named::F12 => Some(Message::GoToDefinition),
            // Ctrl+Espacio en todas las plataformas (fuera de macOS Ctrl es además la
            // tecla de órdenes, así que no se puede exigir que ⌘ no esté pulsado).
            Named::Space if modifiers.control() && !modifiers.alt() && !modifiers.logo() => {
                Some(Message::TriggerCompletion)
            }
            _ => None,
        };
    }

    // ⌘\ divide la pantalla. Se reconoce por la tecla física para que funcione también en
    // teclados donde esa tecla tiene otro carácter (en español es «ç»).
    if modifiers.command() && !modifiers.alt() && physical_key == Physical::Code(Code::Backslash) {
        return Some(Message::ToggleSplit);
    }
    // Listas: ⇧⌘8 viñetas y ⇧⌘7 numerada (como en Pages o Google Docs). Por la tecla
    // física: con Mayús el carácter cambia según el teclado.
    if modifiers.command() && shift && !modifiers.alt() {
        match physical_key {
            Physical::Code(Code::Digit8) => {
                return Some(Message::Context(ContextAction::List(ListKind::Bullet)));
            }
            Physical::Code(Code::Digit7) => {
                return Some(Message::Context(ContextAction::List(ListKind::Numbered)));
            }
            _ => {}
        }
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
        'i' if !shift => Some(Message::ItalicOrHover),
        'b' if !shift => Some(Message::Context(ContextAction::Inline(Inline::Bold))),
        'k' if !shift => Some(Message::Context(ContextAction::Link)),
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
    fn formatting_shortcuts() {
        let cmd = Modifiers::COMMAND;
        assert!(matches!(
            char_key("b", Code::KeyB, cmd),
            Some(Message::Context(ContextAction::Inline(Inline::Bold)))
        ));
        assert!(matches!(
            char_key("k", Code::KeyK, cmd),
            Some(Message::Context(ContextAction::Link))
        ));
        // Con Mayús la tecla 8 da otro carácter («(» en español): cuenta la tecla física.
        assert!(matches!(
            char_key("(", Code::Digit8, cmd | Modifiers::SHIFT),
            Some(Message::Context(ContextAction::List(ListKind::Bullet)))
        ));
        assert!(matches!(
            char_key("/", Code::Digit7, cmd | Modifiers::SHIFT),
            Some(Message::Context(ContextAction::List(ListKind::Numbered)))
        ));
    }

    #[test]
    fn leaving_the_window_ends_a_tab_drag() {
        let window = window::Id::unique();
        for event in [
            iced::mouse::Event::CursorLeft,
            iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left),
        ] {
            assert!(matches!(
                shortcut(Event::Mouse(event), event::Status::Ignored, window),
                Some(Message::MouseReleased)
            ));
        }
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
    fn language_server_shortcuts() {
        assert!(matches!(
            named(Named::F12, Modifiers::empty()),
            Some(Message::GoToDefinition)
        ));
        assert!(matches!(
            named(Named::F8, Modifiers::SHIFT),
            Some(Message::NextProblem(false))
        ));
        assert!(matches!(
            named(Named::Space, Modifiers::CTRL),
            Some(Message::TriggerCompletion)
        ));
        assert!(named(Named::Space, Modifiers::empty()).is_none());
        assert!(matches!(
            char_key("i", Code::KeyI, Modifiers::COMMAND),
            Some(Message::ItalicOrHover)
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
