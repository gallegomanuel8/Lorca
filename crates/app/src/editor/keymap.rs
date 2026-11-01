//! Traducción de pulsaciones de teclado a órdenes del editor.
//!
//! Solo se gestionan aquí las teclas propias de la edición. Los atajos de la aplicación
//! (guardar, deshacer, pestañas…) no se capturan, para que lleguen a la suscripción global.

use iced::keyboard::key::{Named, Physical};
use iced::keyboard::{Key, Modifiers};
use lorca_core::Motion;

use super::EditorAction;

/// Resultado de interpretar una tecla en el editor.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyCommand {
    Edit(EditorAction),
    Copy,
    Cut,
    Paste,
}

/// Interpreta una pulsación. `page_lines` es el número de líneas que avanza Re Pág/Av Pág.
pub fn map_key(
    key: &Key,
    physical_key: Physical,
    modifiers: Modifiers,
    text: Option<&str>,
    page_lines: usize,
) -> Option<KeyCommand> {
    let is_mac = cfg!(target_os = "macos");
    let extend = modifiers.shift();
    // En macOS, Cmd+flecha salta al inicio/fin de línea o documento.
    let line_jump = is_mac && modifiers.logo();
    // Alt en macOS, Ctrl en el resto.
    let word_jump = modifiers.jump();

    let motion = |motion| Some(KeyCommand::Edit(EditorAction::Move { motion, extend }));
    let edit = |action| Some(KeyCommand::Edit(action));

    // ⌥⌘↑/↓ (Ctrl+Alt+↑/↓): añadir cursor arriba o abajo.
    if modifiers.command() && modifiers.alt() {
        match key {
            Key::Named(Named::ArrowUp) => {
                return edit(EditorAction::AddCursorVertical { up: true });
            }
            Key::Named(Named::ArrowDown) => {
                return edit(EditorAction::AddCursorVertical { up: false });
            }
            _ => {}
        }
    }

    if let Key::Named(named) = key {
        match named {
            Named::ArrowLeft => {
                return motion(if line_jump {
                    Motion::LineStart
                } else if word_jump {
                    Motion::WordLeft
                } else {
                    Motion::Left
                });
            }
            Named::ArrowRight => {
                return motion(if line_jump {
                    Motion::LineEnd
                } else if word_jump {
                    Motion::WordRight
                } else {
                    Motion::Right
                });
            }
            Named::ArrowUp => {
                return motion(if line_jump {
                    Motion::DocumentStart
                } else {
                    Motion::Up
                });
            }
            Named::ArrowDown => {
                return motion(if line_jump {
                    Motion::DocumentEnd
                } else {
                    Motion::Down
                });
            }
            Named::Home => {
                return motion(if modifiers.command() {
                    Motion::DocumentStart
                } else {
                    Motion::LineStart
                });
            }
            Named::End => {
                return motion(if modifiers.command() {
                    Motion::DocumentEnd
                } else {
                    Motion::LineEnd
                });
            }
            Named::PageUp => return motion(Motion::PageUp(page_lines)),
            Named::PageDown => return motion(Motion::PageDown(page_lines)),
            Named::Backspace => {
                return edit(if line_jump {
                    EditorAction::DeleteToLineStart
                } else if word_jump {
                    EditorAction::DeleteWordBackward
                } else {
                    EditorAction::Backspace
                });
            }
            Named::Delete => {
                return edit(if word_jump {
                    EditorAction::DeleteWordForward
                } else {
                    EditorAction::Delete
                });
            }
            Named::Enter => return edit(EditorAction::Newline),
            // Ctrl+Tab cambia de pestaña y Mayús+Tab (desindentar) llegará más adelante.
            Named::Tab => {
                let plain =
                    !extend && !modifiers.control() && !modifiers.alt() && !modifiers.logo();
                return plain.then_some(KeyCommand::Edit(EditorAction::Indent));
            }
            Named::Escape => return edit(EditorAction::CollapseSelection),
            // El resto (espacio incluido) sigue como texto.
            _ => {}
        }
    }

    // Combinaciones con Cmd/Ctrl. Ctrl+Alt es AltGr en Windows y produce texto.
    // Con Mayús son atajos de la aplicación (⇧⌘V = vista previa), no del editor.
    if modifiers.command() && !modifiers.alt() {
        if modifiers.shift() {
            return None;
        }
        return match key.to_latin(physical_key) {
            Some('a') => edit(EditorAction::SelectAll),
            Some('d') => edit(EditorAction::AddNextOccurrence),
            Some('c') => Some(KeyCommand::Copy),
            Some('x') => Some(KeyCommand::Cut),
            Some('v') => Some(KeyCommand::Paste),
            _ => None,
        };
    }

    // En macOS, con ⌘ pulsado nunca se escribe: ⌥⌘L, por ejemplo, produciría «¬».
    if cfg!(target_os = "macos") && modifiers.logo() {
        return None;
    }

    let text: String = text?.chars().filter(|c| !c.is_control()).collect();
    (!text.is_empty()).then_some(KeyCommand::Edit(EditorAction::Insert(text)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::key::{Code, NativeCode};

    fn physical() -> Physical {
        Physical::Unidentified(NativeCode::Unidentified)
    }

    fn named(n: Named, modifiers: Modifiers) -> Option<KeyCommand> {
        map_key(&Key::Named(n), physical(), modifiers, None, 10)
    }

    fn character(c: &str, modifiers: Modifiers) -> Option<KeyCommand> {
        map_key(
            &Key::Character(c.into()),
            Physical::Code(Code::KeyA),
            modifiers,
            Some(c),
            10,
        )
    }

    #[test]
    fn plain_text_is_inserted() {
        assert_eq!(
            character("ñ", Modifiers::empty()),
            Some(KeyCommand::Edit(EditorAction::Insert("ñ".into())))
        );
    }

    #[test]
    fn space_is_inserted() {
        let result = map_key(
            &Key::Named(Named::Space),
            physical(),
            Modifiers::empty(),
            Some(" "),
            10,
        );
        assert_eq!(
            result,
            Some(KeyCommand::Edit(EditorAction::Insert(" ".into())))
        );
    }

    #[test]
    fn control_characters_are_ignored() {
        assert_eq!(character("\u{8}", Modifiers::empty()), None);
    }

    #[test]
    fn app_shortcuts_are_not_captured() {
        assert_eq!(character("s", Modifiers::COMMAND), None);
        assert_eq!(character("z", Modifiers::COMMAND), None);
        assert_eq!(named(Named::Tab, Modifiers::CTRL), None);
        assert_eq!(named(Named::Tab, Modifiers::SHIFT), None);
    }

    #[test]
    fn clipboard_shortcuts() {
        assert_eq!(character("c", Modifiers::COMMAND), Some(KeyCommand::Copy));
        assert_eq!(character("x", Modifiers::COMMAND), Some(KeyCommand::Cut));
        assert_eq!(character("v", Modifiers::COMMAND), Some(KeyCommand::Paste));
        assert_eq!(
            character("a", Modifiers::COMMAND),
            Some(KeyCommand::Edit(EditorAction::SelectAll))
        );
    }

    #[test]
    fn shifted_clipboard_combos_are_not_captured() {
        assert_eq!(character("V", Modifiers::COMMAND | Modifiers::SHIFT), None);
        assert_eq!(character("C", Modifiers::COMMAND | Modifiers::SHIFT), None);
    }

    #[test]
    fn multicursor_shortcuts() {
        assert_eq!(
            character("d", Modifiers::COMMAND),
            Some(KeyCommand::Edit(EditorAction::AddNextOccurrence))
        );
        assert_eq!(
            named(Named::ArrowDown, Modifiers::COMMAND | Modifiers::ALT),
            Some(KeyCommand::Edit(EditorAction::AddCursorVertical {
                up: false
            }))
        );
    }

    #[test]
    fn shift_extends_selection() {
        assert_eq!(
            named(Named::ArrowRight, Modifiers::SHIFT),
            Some(KeyCommand::Edit(EditorAction::Move {
                motion: Motion::Right,
                extend: true
            }))
        );
    }

    #[test]
    fn word_motion_uses_platform_modifier() {
        let jump = if cfg!(target_os = "macos") {
            Modifiers::ALT
        } else {
            Modifiers::CTRL
        };
        assert_eq!(
            named(Named::ArrowLeft, jump),
            Some(KeyCommand::Edit(EditorAction::Move {
                motion: Motion::WordLeft,
                extend: false
            }))
        );
    }

    #[test]
    fn page_keys_use_page_size() {
        assert_eq!(
            named(Named::PageDown, Modifiers::empty()),
            Some(KeyCommand::Edit(EditorAction::Move {
                motion: Motion::PageDown(10),
                extend: false
            }))
        );
    }
}
