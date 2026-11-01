//! Barra de menús nativa (solo macOS).
//!
//! En macOS sustituye al menú por defecto de winit, cuyo «Salir» (⌘Q) terminaba el
//! proceso sin avisar: aquí «Salir» pasa por `Message::CloseRequested`, que guarda la
//! sesión. En Windows `muda` necesita código `unsafe` para engancharse a la ventana (el
//! proyecto lo prohíbe) y en Linux, GTK; allí se usan los atajos, la paleta de órdenes y
//! los menús contextuales.

use iced::Subscription;

use crate::app::{BookmarkAction, Message};
use crate::context_menu::ContextAction;

/// Mensaje de la aplicación para el identificador de una opción de menú.
pub fn message_for(id: &str) -> Option<Message> {
    use lorca_formats::Case;
    let message = match id {
        "quit" => Message::CloseRequested,
        "new" => Message::NewDocument,
        "open" => Message::Open,
        "save" => Message::Save,
        "save_as" => Message::SaveAs,
        "export_pdf" => Message::ExportActivePdf,
        "close" => Message::CloseActive,
        "undo" => Message::Undo,
        "redo" => Message::Redo,
        "cut" => Message::Context(ContextAction::Cut),
        "copy" => Message::Context(ContextAction::Copy),
        "paste" => Message::Context(ContextAction::Paste),
        "select_all" => Message::Context(ContextAction::SelectAll),
        "format" => Message::Context(ContextAction::FormatDocument),
        "upper" => Message::Context(ContextAction::ChangeCase(Case::Upper)),
        "lower" => Message::Context(ContextAction::ChangeCase(Case::Lower)),
        "find" => Message::OpenFind,
        "replace" => Message::OpenReplace,
        "find_folder" => Message::FindInFolder,
        "find_next" => Message::FindNext,
        "find_previous" => Message::FindPrevious,
        "goto" => Message::GoToLine,
        "bookmark" => Message::Bookmark(BookmarkAction::Toggle),
        "palette" => Message::OpenPalette,
        "split" => Message::ToggleSplit,
        "preview" => Message::Context(ContextAction::TogglePreview),
        "whitespace" => Message::Context(ContextAction::ToggleWhitespace),
        "line_endings" => Message::Context(ContextAction::ToggleLineEndings),
        "guides" => Message::Context(ContextAction::ToggleIndentGuides),
        "zoom_in" => Message::Zoom(1),
        "zoom_out" => Message::Zoom(-1),
        "zoom_reset" => Message::Zoom(0),
        "open_config" => Message::OpenConfig,
        "new_theme" => Message::NewTheme,
        _ => return None,
    };
    Some(message)
}

#[cfg(target_os = "macos")]
pub use native::{NativeMenu, events, install};

#[cfg(not(target_os = "macos"))]
pub use fallback::{NativeMenu, events, install};

#[cfg(target_os = "macos")]
mod native {
    use std::fmt;

    use iced::futures::{SinkExt, Stream, StreamExt};
    use muda::accelerator::Accelerator;
    use muda::{AboutMetadata, IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

    use super::{Message, Subscription, message_for};

    /// Menú instalado. Hay que conservarlo vivo mientras dure la aplicación.
    pub struct NativeMenu {
        _menu: Menu,
    }

    impl fmt::Debug for NativeMenu {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("NativeMenu")
        }
    }

    /// Opción con identificador y atajo opcional (sintaxis de `muda`, p. ej. «CmdOrCtrl+S»).
    fn item(id: &str, label: &str, accelerator: Option<&str>) -> MenuItem {
        let accelerator = accelerator.and_then(|a| a.parse::<Accelerator>().ok());
        MenuItem::with_id(id, label, true, accelerator)
    }

    fn submenu(label: &str, items: &[&dyn IsMenuItem]) -> Result<Submenu, String> {
        Submenu::with_items(label, true, items).map_err(|e| e.to_string())
    }

    /// Crea e instala la barra de menús. Debe llamarse en el hilo principal, después de
    /// que la aplicación haya arrancado (así sustituye al menú por defecto de winit).
    pub fn install() -> Result<NativeMenu, String> {
        let separator = PredefinedMenuItem::separator;
        let about = AboutMetadata {
            name: Some("Lorca".to_owned()),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            comments: Some("Verde que te quiero verde.".to_owned()),
            ..AboutMetadata::default()
        };
        let app = submenu(
            "Lorca",
            &[
                &PredefinedMenuItem::about(Some("Acerca de Lorca"), Some(about)),
                &separator(),
                &item("open_config", "Ajustes…", Some("CmdOrCtrl+Comma")),
                &separator(),
                &PredefinedMenuItem::services(Some("Servicios")),
                &separator(),
                &PredefinedMenuItem::hide(Some("Ocultar Lorca")),
                &PredefinedMenuItem::hide_others(Some("Ocultar otros")),
                &PredefinedMenuItem::show_all(Some("Mostrar todo")),
                &separator(),
                &item("quit", "Salir de Lorca", Some("CmdOrCtrl+Q")),
            ],
        )?;
        let file = submenu(
            "Archivo",
            &[
                &item("new", "Nuevo", Some("CmdOrCtrl+N")),
                &item("open", "Abrir…", Some("CmdOrCtrl+O")),
                &separator(),
                &item("save", "Guardar", Some("CmdOrCtrl+S")),
                &item("save_as", "Guardar como…", Some("CmdOrCtrl+Shift+S")),
                &item("export_pdf", "Exportar a PDF…", None),
                &separator(),
                &item("close", "Cerrar pestaña", Some("CmdOrCtrl+W")),
            ],
        )?;
        // Copiar, cortar y pegar van sin atajo: si el menú se quedara con ⌘C/⌘V, los
        // campos de texto (búsqueda, paleta) no los recibirían.
        let edit = submenu(
            "Editar",
            &[
                &item("undo", "Deshacer", Some("CmdOrCtrl+Z")),
                &item("redo", "Rehacer", Some("CmdOrCtrl+Shift+Z")),
                &separator(),
                &item("cut", "Cortar", None),
                &item("copy", "Copiar", None),
                &item("paste", "Pegar", None),
                &item("select_all", "Seleccionar todo", None),
                &separator(),
                &item("format", "Formatear documento", Some("Alt+CmdOrCtrl+L")),
                &item("upper", "MAYÚSCULAS", Some("CmdOrCtrl+Shift+U")),
                &item("lower", "minúsculas", Some("CmdOrCtrl+U")),
            ],
        )?;
        let search = submenu(
            "Buscar",
            &[
                &item("find", "Buscar…", Some("CmdOrCtrl+F")),
                &item("replace", "Reemplazar…", Some("Alt+CmdOrCtrl+F")),
                &item(
                    "find_folder",
                    "Buscar en carpeta…",
                    Some("CmdOrCtrl+Shift+F"),
                ),
                &separator(),
                &item("find_next", "Buscar siguiente", Some("CmdOrCtrl+G")),
                &item(
                    "find_previous",
                    "Buscar anterior",
                    Some("CmdOrCtrl+Shift+G"),
                ),
                &separator(),
                &item("goto", "Ir a línea…", Some("Ctrl+G")),
                &item("bookmark", "Poner o quitar marcador", Some("CmdOrCtrl+F2")),
            ],
        )?;
        let view = submenu(
            "Ver",
            &[
                &item("palette", "Paleta de órdenes…", Some("CmdOrCtrl+Shift+P")),
                &separator(),
                &item("split", "Dividir pantalla / quitar división", None),
                &item(
                    "preview",
                    "Vista previa de Markdown",
                    Some("CmdOrCtrl+Shift+V"),
                ),
                &separator(),
                &item("whitespace", "Mostrar espacios y tabuladores", None),
                &item("line_endings", "Mostrar finales de línea", None),
                &item("guides", "Guías de indentación", None),
                &separator(),
                &item("zoom_in", "Aumentar letra", None),
                &item("zoom_out", "Reducir letra", None),
                &item("zoom_reset", "Tamaño de letra normal", None),
                &separator(),
                &item("new_theme", "Crear tema a partir del actual", None),
            ],
        )?;
        let window = submenu(
            "Ventana",
            &[
                &PredefinedMenuItem::minimize(Some("Minimizar")),
                &PredefinedMenuItem::maximize(Some("Zoom")),
                &PredefinedMenuItem::fullscreen(Some("Pantalla completa")),
            ],
        )?;

        let menu = Menu::new();
        for submenu in [&app, &file, &edit, &search, &view, &window] {
            menu.append(submenu).map_err(|e| e.to_string())?;
        }
        window.set_as_windows_menu_for_nsapp();
        menu.init_for_nsapp();
        Ok(NativeMenu { _menu: menu })
    }

    /// Mensajes de las opciones de menú elegidas.
    pub fn events() -> Subscription<Message> {
        Subscription::run(stream)
    }

    fn stream() -> impl Stream<Item = Message> {
        iced::stream::channel(32, async |mut output| {
            let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded::<String>();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let _ = sender.unbounded_send(event.id().0.clone());
            }));
            while let Some(id) = receiver.next().await {
                if let Some(message) = message_for(&id)
                    && output.send(message).await.is_err()
                {
                    break;
                }
            }
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod fallback {
    use super::{Message, Subscription};

    /// Sin barra de menús nativa fuera de macOS.
    #[derive(Debug)]
    pub struct NativeMenu;

    pub fn install() -> Result<NativeMenu, String> {
        Ok(NativeMenu)
    }

    pub fn events() -> Subscription<Message> {
        Subscription::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_id_has_a_message() {
        for id in [
            "quit",
            "new",
            "open",
            "save",
            "save_as",
            "export_pdf",
            "close",
            "undo",
            "redo",
            "cut",
            "copy",
            "paste",
            "select_all",
            "format",
            "upper",
            "lower",
            "find",
            "replace",
            "find_folder",
            "find_next",
            "find_previous",
            "goto",
            "bookmark",
            "palette",
            "split",
            "preview",
            "whitespace",
            "line_endings",
            "guides",
        ] {
            assert!(message_for(id).is_some(), "sin mensaje: {id}");
        }
        assert!(message_for("desconocido").is_none());
    }

    #[test]
    fn quit_goes_through_close_request() {
        assert!(matches!(message_for("quit"), Some(Message::CloseRequested)));
    }
}
