//! Barra de menús: nativa en macOS y dibujada con iced en Windows y Linux.
//!
//! En macOS sustituye al menú por defecto de winit, cuyo «Salir» (⌘Q) terminaba el
//! proceso sin avisar: aquí «Salir» pasa por `Message::CloseRequested`, que guarda la
//! sesión. En Windows `muda` necesita código `unsafe` para engancharse a la ventana (el
//! proyecto lo prohíbe) y en Linux, GTK; allí se dibuja una barra propia ([`bar`]) cuyos
//! desplegables son menús contextuales (`MenuTarget::MenuBar`).

use iced::widget::{container, row, text};
use iced::{Element, Subscription, Theme};

use crate::app::{BookmarkAction, Message};
use crate::context_menu::{ContextAction, MenuTarget, anchor_area};
use crate::shortcut_labels as keys;

/// Alto de la barra de menús dibujada.
pub const BAR_HEIGHT: f32 = 26.0;

/// Elemento de un menú de la barra dibujada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarEntry {
    /// Identificador de [`message_for`], texto y atajo.
    Item(&'static str, &'static str, &'static str),
    Separator,
}

/// Menú de la barra dibujada: título y elementos.
#[derive(Debug, Clone, Copy)]
pub struct BarMenu {
    pub title: &'static str,
    pub entries: &'static [BarEntry],
}

use BarEntry::{Item, Separator};

/// Menús de la barra dibujada (Windows y Linux), con los mismos identificadores que la
/// barra nativa de macOS.
pub const BAR: &[BarMenu] = &[
    BarMenu {
        title: "Archivo",
        entries: &[
            Item("new", "Nuevo", keys::NEW),
            Item("open", "Abrir…", keys::OPEN),
            Separator,
            Item("save", "Guardar", keys::SAVE),
            Item("save_as", "Guardar como…", keys::SAVE_AS),
            Item("export_pdf", "Exportar a PDF…", ""),
            Separator,
            Item("open_config", "Ajustes…", keys::SETTINGS),
            Item("check_updates", "Buscar actualizaciones…", ""),
            Separator,
            Item("close", "Cerrar pestaña", keys::CLOSE),
            Item("quit", "Salir", keys::QUIT),
        ],
    },
    BarMenu {
        title: "Editar",
        entries: &[
            Item("undo", "Deshacer", keys::UNDO),
            Item("redo", "Rehacer", keys::REDO),
            Separator,
            Item("cut", "Cortar", keys::CUT),
            Item("copy", "Copiar", keys::COPY),
            Item("paste", "Pegar", keys::PASTE),
            Item("select_all", "Seleccionar todo", keys::SELECT_ALL),
            Separator,
            Item("format", "Formatear documento", keys::FORMAT),
            Item("upper", "MAYÚSCULAS", keys::UPPER),
            Item("lower", "minúsculas", keys::LOWER),
            Separator,
            Item("bold", "Negrita", keys::BOLD),
            Item("italic", "Cursiva", keys::ITALIC),
            Item("bullets", "Lista con viñetas", keys::BULLETS),
            Item("numbered", "Lista numerada", keys::NUMBERED),
            Item("strikethrough", "Tachado", ""),
            Item("link", "Insertar vínculo", keys::LINK),
            Item("clear_format", "Borrar formato", ""),
        ],
    },
    BarMenu {
        title: "Buscar",
        entries: &[
            Item("find", "Buscar…", keys::FIND),
            Item("replace", "Reemplazar…", keys::REPLACE),
            Item("find_folder", "Buscar en carpeta…", keys::FIND_IN_FOLDER),
            Separator,
            Item("find_next", "Buscar siguiente", keys::FIND_NEXT),
            Item("find_previous", "Buscar anterior", keys::FIND_PREVIOUS),
            Separator,
            Item("goto", "Ir a línea…", keys::GOTO),
            Item("bookmark", "Poner o quitar marcador", keys::BOOKMARK),
        ],
    },
    BarMenu {
        title: "Código",
        entries: &[
            Item("complete", "Autocompletar", keys::COMPLETE),
            Item("hover", "Mostrar información", keys::HOVER),
            Item("definition", "Ir a la definición", keys::DEFINITION),
            Separator,
            Item("next_problem", "Siguiente problema", keys::NEXT_PROBLEM),
            Item(
                "previous_problem",
                "Problema anterior",
                keys::PREVIOUS_PROBLEM,
            ),
            Separator,
            Item("lsp_restart", "Reiniciar servidores de lenguaje", ""),
        ],
    },
    BarMenu {
        title: "Ver",
        entries: &[
            Item("palette", "Paleta de órdenes…", keys::PALETTE),
            Separator,
            Item("split", "Dividir pantalla / quitar división", keys::SPLIT),
            Item("preview", "Vista previa de Markdown", keys::PREVIEW),
            Separator,
            Item("whitespace", "Espacios y tabuladores", ""),
            Item("line_endings", "Finales de línea", ""),
            Item("guides", "Guías de indentación", ""),
            Item("wrap", "Ajuste de línea al ver las marcas", ""),
            Item("markers", "Mostrar marcas de Markdown", ""),
            Separator,
            Item("zoom_in", "Aumentar letra", keys::ZOOM_IN),
            Item("zoom_out", "Reducir letra", keys::ZOOM_OUT),
            Item("zoom_reset", "Tamaño de letra normal", keys::ZOOM_RESET),
            Separator,
            Item("new_theme", "Crear tema a partir del actual", ""),
        ],
    },
];

/// Barra de menús dibujada. `open` es el índice del menú desplegado, si lo hay.
pub fn bar<'a>(open: Option<usize>) -> Element<'a, Message> {
    let titles = BAR.iter().enumerate().map(|(index, menu)| {
        let highlighted = open == Some(index);
        let title = container(text(menu.title).size(13))
            .height(BAR_HEIGHT)
            .padding([4, 10])
            .style(move |theme: &Theme| {
                let palette = theme.extended_palette();
                if highlighted {
                    container::Style::default()
                        .background(palette.primary.weak.color)
                        .color(palette.primary.weak.text)
                } else {
                    container::Style::default()
                }
            });
        anchor_area(title, move |position| {
            Message::StatusMenu(MenuTarget::MenuBar(index), position)
        })
    });
    container(row(titles))
        .width(iced::Length::Fill)
        .style(|theme: &Theme| {
            container::Style::default().background(theme.extended_palette().background.weak.color)
        })
        .into()
}

/// Mensaje de la aplicación para el identificador de una opción de menú.
pub fn message_for(id: &str) -> Option<Message> {
    use lorca_formats::{Case, Inline, ListKind};
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
        "bold" => Message::Context(ContextAction::Inline(Inline::Bold)),
        "italic" => Message::Context(ContextAction::Inline(Inline::Italic)),
        "bullets" => Message::Context(ContextAction::List(ListKind::Bullet)),
        "numbered" => Message::Context(ContextAction::List(ListKind::Numbered)),
        "strikethrough" => Message::Context(ContextAction::Inline(Inline::Strikethrough)),
        "link" => Message::Context(ContextAction::Link),
        "clear_format" => Message::Context(ContextAction::ClearFormatting),
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
        "wrap" => Message::Context(ContextAction::ToggleWordWrap),
        "markers" => Message::Context(ContextAction::ToggleMarkdownMarkers),
        "zoom_in" => Message::Zoom(1),
        "zoom_out" => Message::Zoom(-1),
        "zoom_reset" => Message::Zoom(0),
        "open_config" => Message::OpenConfig,
        "check_updates" => Message::CheckUpdates { manual: true },
        "new_theme" => Message::NewTheme,
        "complete" => Message::TriggerCompletion,
        "hover" => Message::ShowHover,
        "definition" => Message::GoToDefinition,
        "next_problem" => Message::NextProblem(true),
        "previous_problem" => Message::NextProblem(false),
        "lsp_restart" => Message::LspRestart,
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

    /// Retrato de Lorca hecho con caracteres: lo muestra «Acerca de» (el icono de la
    /// aplicación es otro, más fácil de distinguir en el Dock).
    const PORTRAIT: &[u8] = include_bytes!("../../../assets/icons/retrato/lorca-512.png");

    fn portrait() -> Option<muda::Icon> {
        let image = image::load_from_memory_with_format(PORTRAIT, image::ImageFormat::Png)
            .ok()?
            .into_rgba8();
        let (width, height) = image.dimensions();
        muda::Icon::from_rgba(image.into_raw(), width, height).ok()
    }

    /// Crea e instala la barra de menús. Debe llamarse en el hilo principal, después de
    /// que la aplicación haya arrancado (así sustituye al menú por defecto de winit).
    pub fn install() -> Result<NativeMenu, String> {
        let separator = PredefinedMenuItem::separator;
        let about = AboutMetadata {
            name: Some("Lorca".to_owned()),
            version: Some(env!("CARGO_PKG_VERSION").to_owned()),
            comments: Some("Verde que te quiero verde.".to_owned()),
            icon: portrait(),
            ..AboutMetadata::default()
        };
        let app = submenu(
            "Lorca",
            &[
                &PredefinedMenuItem::about(Some("Acerca de Lorca"), Some(about)),
                &item("check_updates", "Buscar actualizaciones…", None),
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
                &separator(),
                // Sin atajo en el menú: ⌘I es cursiva solo en texto (en código, información).
                &item("bold", "Negrita", None),
                &item("italic", "Cursiva", None),
                &item("bullets", "Lista con viñetas", None),
                &item("numbered", "Lista numerada", None),
                &item("strikethrough", "Tachado", None),
                &item("link", "Insertar vínculo", None),
                &item("clear_format", "Borrar formato", None),
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
        let code = submenu(
            "Código",
            &[
                &item("complete", "Autocompletar", None),
                &item("hover", "Mostrar información", None),
                &item("definition", "Ir a la definición", None),
                &separator(),
                &item("next_problem", "Siguiente problema", None),
                &item("previous_problem", "Problema anterior", None),
                &separator(),
                &item("lsp_restart", "Reiniciar servidores de lenguaje", None),
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
                &item("wrap", "Ajuste de línea al ver las marcas", None),
                &item("markers", "Mostrar marcas de Markdown", None),
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
        for submenu in [&app, &file, &edit, &search, &code, &view, &window] {
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
            "bold",
            "italic",
            "bullets",
            "numbered",
            "strikethrough",
            "link",
            "clear_format",
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
            "wrap",
            "markers",
            "zoom_in",
            "zoom_out",
            "zoom_reset",
            "open_config",
            "check_updates",
            "new_theme",
            "complete",
            "hover",
            "definition",
            "next_problem",
            "previous_problem",
            "lsp_restart",
        ] {
            assert!(message_for(id).is_some(), "sin mensaje: {id}");
        }
        assert!(message_for("desconocido").is_none());
    }

    #[test]
    fn every_bar_item_has_a_message() {
        for menu in BAR {
            for entry in menu.entries {
                if let BarEntry::Item(id, _, _) = entry {
                    assert!(message_for(id).is_some(), "sin mensaje: {id}");
                }
            }
        }
    }

    #[test]
    fn quit_goes_through_close_request() {
        assert!(matches!(message_for("quit"), Some(Message::CloseRequested)));
    }
}
