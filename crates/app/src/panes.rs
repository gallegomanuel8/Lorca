//! Pantalla dividida, selección de pestañas y menús contextuales sobre `Lorca`.
//!
//! Es un submódulo de `app` para poder usar su estado privado.

use std::path::Path;

use iced::widget::{
    button, column, container, markdown, pane_grid, row, rule, scrollable, space, text,
};
use iced::{Alignment, Element, Length, Point, Task, Theme};
use lorca_core::{Buffer, LineEnding, Position, Selection, TextEncoding, TextFormat};
use lorca_formats::{Case, change_case, format_json, format_sql, minify_json};
use lorca_search::match_index;
use lorca_syntax::Language;
use lorca_workspace::DocumentId;

use super::{Lorca, Message, Then};
use crate::context_menu::{ContextAction, ContextMenu, MenuItem, MenuTarget};
use crate::editor::{Decorations, ViewOptions, code_editor};
use crate::find;
use crate::palette::{self, Command};
use crate::shortcut_labels as keys;

/// Paneles de iced y su orden de izquierda a derecha.
#[derive(Debug)]
pub struct PaneLayout {
    pub state: pane_grid::State<DocumentId>,
    order: Vec<pane_grid::Pane>,
}

impl PaneLayout {
    /// Paneles lado a lado y del mismo ancho para los documentos indicados.
    fn new(ids: &[DocumentId]) -> Option<Self> {
        let (&first, rest) = ids.split_first()?;
        let (mut state, first_pane) = pane_grid::State::new(first);
        let mut order = vec![first_pane];
        let mut last = first_pane;
        let total = ids.len();
        for (index, id) in rest.iter().enumerate() {
            let (pane, split) = state.split(pane_grid::Axis::Vertical, last, *id)?;
            // Cada división separa un panel del resto: 1/n, 1/(n-1)… da anchos iguales.
            state.resize(split, 1.0 / (total - index) as f32);
            order.push(pane);
            last = pane;
        }
        Some(Self { state, order })
    }
}

impl Lorca {
    /// Mantiene los paneles de iced al día con `Workspace::panes`, conservando los anchos
    /// si solo cambia qué documento muestra cada panel.
    pub(super) fn sync_panes(&mut self) {
        let ids = self.workspace.panes();
        if ids.is_empty() {
            self.pane_layout = None;
            return;
        }
        match &mut self.pane_layout {
            Some(layout) if layout.order.len() == ids.len() => {
                for (pane, id) in layout.order.iter().zip(ids) {
                    if let Some(slot) = layout.state.get_mut(*pane) {
                        *slot = *id;
                    }
                }
            }
            _ => self.pane_layout = PaneLayout::new(ids),
        }
    }

    /// Documentos visibles: los de los paneles o, sin dividir, el activo.
    pub(super) fn visible_documents(&self) -> Vec<DocumentId> {
        if self.workspace.is_split() {
            self.workspace.panes().to_vec()
        } else {
            self.workspace.active_id().into_iter().collect()
        }
    }

    /// Editor de un documento con su resaltado, coincidencias y marcas.
    pub(super) fn editor_for(&self, id: DocumentId) -> Element<'_, Message> {
        let Some(document) = self.workspace.document(id) else {
            return text("").into();
        };
        let is_active = self.workspace.active_id() == Some(id);
        let view = self.views.get(&id).copied().unwrap_or_default();
        let highlights = self
            .highlights
            .get(&id)
            .and_then(|s| s.highlights.as_deref());
        // Las coincidencias de la búsqueda solo se muestran en el documento activo.
        let matches = match (&self.find_cache, is_active) {
            (Some(cache), true) => cache.matches.as_slice(),
            _ => &[],
        };
        let decorations = Decorations {
            matches,
            current: match_index(matches, &document.editor.selection().range()),
            marks: self
                .mark_cache
                .get(&id)
                .map_or(&[][..], |c| c.matches.as_slice()),
            underlines: self.underlines(id),
        };
        let focused = is_active
            && self.focus == super::Focus::Editor
            && !self.dialog_open
            && self.context_menu.is_none()
            && self.palette.is_none();
        let options = ViewOptions {
            whitespace: self.config.show_whitespace,
            line_ending: self
                .config
                .show_line_endings
                .then(|| document.format().line_ending),
            indent_guides: self.config.indent_guides,
            metrics: self.metrics(),
            completion_open: self
                .lsp
                .completion
                .as_ref()
                .is_some_and(|c| c.document == id),
        };
        let editor: Element<'_, Message> = code_editor(
            &document.editor,
            view,
            focused,
            highlights,
            decorations,
            options,
            move |action| Message::Editor(id, action),
        )
        .colors(&self.active_theme.editor)
        .into();
        match self.preview_for(id) {
            Some(preview) => row![editor, rule::vertical(1), preview].into(),
            None => editor,
        }
    }

    /// Vista previa de Markdown del documento, si está activada.
    fn preview_for(&self, id: DocumentId) -> Option<Element<'_, Message>> {
        if !self.previews.contains(&id) {
            return None;
        }
        let (_, content) = self.preview_cache.get(&id)?;
        let settings = markdown::Settings::with_text_size(15, self.theme());
        let rendered = markdown::view(content.items(), settings).map(Message::OpenUrl);
        let header = row![
            text("Vista previa").size(12),
            space::horizontal(),
            button(text("Exportar PDF").size(12))
                .padding([3, 10])
                .style(button::secondary)
                .on_press(Message::ExportPdf(id)),
        ]
        .align_y(Alignment::Center);
        let header = container(header)
            .padding([4, 10])
            .width(Length::Fill)
            .style(|theme: &Theme| {
                container::Style::default()
                    .background(theme.extended_palette().background.weakest.color)
            });
        let body = scrollable(container(rendered).padding([16, 22]).width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill);
        Some(column![header, body].width(Length::Fill).into())
    }

    /// Vuelve a analizar el Markdown de las vistas previas visibles que hayan cambiado.
    pub(super) fn refresh_previews(&mut self) {
        let workspace = &self.workspace;
        self.previews.retain(|id| workspace.document(*id).is_some());
        let previews = &self.previews;
        self.preview_cache.retain(|id, _| previews.contains(id));
        for id in self.visible_documents() {
            if !self.previews.contains(&id) {
                continue;
            }
            let Some(document) = self.workspace.document(id) else {
                continue;
            };
            let revision = document.editor.version();
            if self
                .preview_cache
                .get(&id)
                .is_some_and(|(cached, _)| *cached == revision)
            {
                continue;
            }
            let content = markdown::Content::parse(&document.editor.text());
            self.preview_cache.insert(id, (revision, content));
        }
    }

    /// Área de edición: un editor o, con la pantalla dividida, varios lado a lado.
    pub(super) fn editor_area(&self) -> Element<'_, Message> {
        let Some(layout) = &self.pane_layout else {
            return match self.workspace.active_id() {
                Some(id) => self.editor_for(id),
                None => text("").into(),
            };
        };
        let active = self.workspace.active_id();
        pane_grid(&layout.state, |_pane, id, _maximized| {
            let id = *id;
            let is_active = active == Some(id);
            let name = self.workspace.document(id).map_or_else(String::new, |d| {
                let modified = if d.is_modified() { " ●" } else { "" };
                format!("{}{modified}", d.display_name())
            });
            let title = container(text(name).size(12))
                .padding([3, 10])
                .width(Length::Fill)
                .style(move |theme: &Theme| {
                    let palette = theme.extended_palette();
                    let (background, color) = if is_active {
                        (palette.primary.weak.color, palette.primary.weak.text)
                    } else {
                        (
                            palette.background.weakest.color,
                            palette.background.strong.text,
                        )
                    };
                    container::Style::default()
                        .background(background)
                        .color(color)
                });
            pane_grid::Content::new(column![title, self.editor_for(id)])
        })
        .spacing(4)
        .on_click(Message::PaneClicked)
        .on_resize(8, Message::PaneResized)
        .into()
    }

    // --- Paleta de órdenes -------------------------------------------------------------

    /// Órdenes de la paleta que encajan con lo escrito.
    pub(super) fn palette_matches(&self) -> Vec<Command> {
        let query = self.palette.as_ref().map_or("", |p| p.query.as_str());
        palette::filter(self.palette_commands(), query)
    }

    /// Todas las órdenes disponibles ahora mismo.
    fn palette_commands(&self) -> Vec<Command> {
        use super::BookmarkAction;
        let context = Message::Context;
        let mut commands = vec![
            Command::new("Archivo: Nuevo documento", keys::NEW, Message::NewDocument),
            Command::new("Archivo: Abrir…", keys::OPEN, Message::Open),
            Command::new("Archivo: Guardar", keys::SAVE, Message::Save),
            Command::new("Archivo: Guardar como…", keys::SAVE_AS, Message::SaveAs),
            Command::new("Archivo: Cerrar pestaña", keys::CLOSE, Message::CloseActive),
            Command::new("Editar: Deshacer", keys::UNDO, Message::Undo),
            Command::new("Editar: Rehacer", keys::REDO, Message::Redo),
            Command::new(
                "Editar: Seleccionar todo",
                keys::SELECT_ALL,
                context(ContextAction::SelectAll),
            ),
            Command::new(
                "Editar: MAYÚSCULAS",
                keys::UPPER,
                context(ContextAction::ChangeCase(Case::Upper)),
            ),
            Command::new(
                "Editar: minúsculas",
                keys::LOWER,
                context(ContextAction::ChangeCase(Case::Lower)),
            ),
            Command::new(
                "Editar: Tipo Título",
                "",
                context(ContextAction::ChangeCase(Case::Title)),
            ),
            Command::new(
                "Editar: Formatear documento (JSON/SQL)",
                keys::FORMAT,
                context(ContextAction::FormatDocument),
            ),
            Command::new(
                "Editar: Minificar JSON",
                "",
                context(ContextAction::MinifyJson),
            ),
            Command::new("Buscar: Buscar…", keys::FIND, Message::OpenFind),
            Command::new("Buscar: Reemplazar…", keys::REPLACE, Message::OpenReplace),
            Command::new(
                "Buscar: Buscar en carpeta…",
                keys::FIND_IN_FOLDER,
                Message::FindInFolder,
            ),
            Command::new("Buscar: Ir a línea…", keys::GOTO, Message::GoToLine),
            Command::new(
                "Marcadores: Poner o quitar",
                keys::BOOKMARK,
                Message::Bookmark(BookmarkAction::Toggle),
            ),
            Command::new(
                "Marcadores: Siguiente",
                "F2",
                Message::Bookmark(BookmarkAction::Next),
            ),
            Command::new(
                "Marcadores: Anterior",
                "⇧F2",
                Message::Bookmark(BookmarkAction::Previous),
            ),
            Command::new(
                "Ver: Dividir pantalla / quitar división",
                keys::SPLIT,
                Message::ToggleSplit,
            ),
            Command::new(
                "Ver: Vista previa de Markdown",
                keys::PREVIEW,
                context(ContextAction::TogglePreview),
            ),
            Command::new(
                "Ver: Mostrar espacios y tabuladores",
                "",
                context(ContextAction::ToggleWhitespace),
            ),
            Command::new(
                "Ver: Mostrar finales de línea",
                "",
                context(ContextAction::ToggleLineEndings),
            ),
            Command::new(
                "Ver: Guías de indentación",
                "",
                context(ContextAction::ToggleIndentGuides),
            ),
            Command::new(
                "Finales de línea: LF",
                "",
                context(ContextAction::SetLineEnding(LineEnding::Lf)),
            ),
            Command::new(
                "Finales de línea: CRLF",
                "",
                context(ContextAction::SetLineEnding(LineEnding::CrLf)),
            ),
        ];
        if let Some(id) = self.workspace.active_id() {
            commands.push(Command::new(
                "Archivo: Exportar a PDF…",
                "",
                Message::ExportPdf(id),
            ));
        }
        for mode in lorca_workspace::ThemeMode::ALL {
            commands.push(Command::new(
                format!("Tema: modo {}", mode.label().to_lowercase()),
                "",
                Message::ThemeSelected(mode),
            ));
        }
        commands.extend([
            Command::new(
                "Código: Autocompletar",
                keys::COMPLETE,
                Message::TriggerCompletion,
            ),
            Command::new(
                "Código: Mostrar información",
                keys::HOVER,
                Message::ShowHover,
            ),
            Command::new("Código: Ir a la definición", "F12", Message::GoToDefinition),
            Command::new(
                "Código: Siguiente problema",
                "F8",
                Message::NextProblem(true),
            ),
            Command::new(
                "Código: Problema anterior",
                "⇧F8",
                Message::NextProblem(false),
            ),
            Command::new("LSP: Reiniciar servidores", "", Message::LspRestart),
            Command::new("Ver: Aumentar letra", keys::ZOOM_IN, Message::Zoom(1)),
            Command::new("Ver: Reducir letra", keys::ZOOM_OUT, Message::Zoom(-1)),
            Command::new(
                "Ver: Tamaño de letra normal",
                keys::ZOOM_RESET,
                Message::Zoom(0),
            ),
            Command::new(
                "Preferencias: Abrir configuración (config.toml)",
                keys::SETTINGS,
                Message::OpenConfig,
            ),
            Command::new(
                "Preferencias: Crear tema a partir del actual",
                "",
                Message::NewTheme,
            ),
        ]);
        for (index, spec) in self.themes.iter().enumerate() {
            commands.push(Command::new(
                format!("Tema: {}", spec.name),
                "",
                context(ContextAction::SetTheme(index)),
            ));
        }
        for language in Language::ALL {
            commands.push(Command::new(
                format!("Lenguaje: {}", language.name()),
                "",
                Message::LanguageSelected(language),
            ));
        }
        for document in self.workspace.documents() {
            commands.push(Command::new(
                format!("Ir a pestaña: {}", document.display_name()),
                "",
                Message::ActivateTab(document.id()),
            ));
        }
        commands
    }

    // --- Pestañas ---------------------------------------------------------------------

    /// Clic en una pestaña: con Cmd/Ctrl añade a la selección, con Mayús selecciona un
    /// rango y sin modificadores la activa.
    pub(super) fn tab_pressed(&mut self, id: DocumentId) {
        if self.modifiers.command() {
            self.workspace.toggle_selection(id);
        } else if self.modifiers.shift() {
            self.workspace.select_range(id);
        } else {
            self.workspace.clear_selection();
            self.workspace.activate(id);
        }
    }

    /// Divide con las pestañas seleccionadas o, si ya está dividida, vuelve a una vista.
    pub(super) fn toggle_split(&mut self) {
        if self.workspace.selected().len() >= 2 {
            let selected = self.workspace.selected().to_vec();
            self.workspace.split(&selected);
        } else if self.workspace.is_split() {
            self.workspace.unsplit();
        } else {
            self.show_info(format!(
                "Selecciona de 2 a 4 pestañas con {}+clic para dividir la pantalla",
                keys::COMMAND
            ));
        }
    }

    // --- Menús contextuales -----------------------------------------------------------

    /// Elige el tema `index` para su aspecto y, si no es el que se ve ahora, cambia el
    /// modo para que se vea.
    fn select_theme(&mut self, index: usize) {
        let Some(spec) = self.themes.iter().nth(index) else {
            return;
        };
        let appearance = spec.appearance;
        let name = spec.name.clone();
        match appearance {
            lorca_workspace::Appearance::Light => self.config.light_theme = name,
            lorca_workspace::Appearance::Dark => self.config.dark_theme = name,
        }
        if self.appearance() != appearance {
            self.config.theme = Some(match appearance {
                lorca_workspace::Appearance::Light => lorca_workspace::ThemeMode::Light,
                lorca_workspace::Appearance::Dark => lorca_workspace::ThemeMode::Dark,
            });
        }
        self.save_config();
    }

    pub(super) fn open_context_menu(&mut self, position: Point, target: MenuTarget) {
        if let MenuTarget::Text(id) = target {
            self.workspace.activate(id);
        }
        self.context_menu = Some(ContextMenu { position, target });
    }

    /// Elementos del menú según dónde se abrió.
    pub(super) fn menu_items(&self, target: MenuTarget) -> Vec<MenuItem> {
        let some = |enabled: bool, action| enabled.then_some(action);
        match target {
            MenuTarget::Text(id) => {
                let Some(document) = self.workspace.document(id) else {
                    return Vec::new();
                };
                let editor = &document.editor;
                let has_selection = !editor.selection().is_empty();
                let language = document.language();
                let mut items = Vec::new();
                if self.lsp_status().is_some_and(|(_, ready)| ready) {
                    items.extend([
                        MenuItem::new(
                            "Ir a la definición",
                            "F12",
                            Some(ContextAction::GoToDefinition),
                        ),
                        MenuItem::new(
                            "Mostrar información",
                            keys::HOVER,
                            Some(ContextAction::ShowHover),
                        ),
                        MenuItem::Separator,
                    ]);
                }
                items.extend([
                    MenuItem::new("Cortar", keys::CUT, some(has_selection, ContextAction::Cut)),
                    MenuItem::new(
                        "Copiar",
                        keys::COPY,
                        some(has_selection, ContextAction::Copy),
                    ),
                    MenuItem::new("Pegar", keys::PASTE, Some(ContextAction::Paste)),
                    MenuItem::new(
                        "Eliminar",
                        "Supr",
                        some(has_selection, ContextAction::Delete),
                    ),
                    MenuItem::new(
                        "Seleccionar todo",
                        keys::SELECT_ALL,
                        Some(ContextAction::SelectAll),
                    ),
                    MenuItem::Separator,
                    MenuItem::new(
                        "Deshacer",
                        keys::UNDO,
                        some(editor.can_undo(), ContextAction::Undo),
                    ),
                    MenuItem::new(
                        "Rehacer",
                        keys::REDO,
                        some(editor.can_redo(), ContextAction::Redo),
                    ),
                    MenuItem::Separator,
                ]);
                let format_label = match language {
                    Language::Json => "Formatear JSON",
                    Language::Sql => "Formatear SQL",
                    _ => "Formatear (JSON o SQL)",
                };
                items.push(MenuItem::new(
                    format_label,
                    keys::FORMAT,
                    some(
                        matches!(language, Language::Json | Language::Sql),
                        ContextAction::FormatDocument,
                    ),
                ));
                if language == Language::Json {
                    items.push(MenuItem::new(
                        "Minificar JSON",
                        "",
                        Some(ContextAction::MinifyJson),
                    ));
                }
                items.extend([
                    MenuItem::new(
                        "MAYÚSCULAS",
                        keys::UPPER,
                        some(has_selection, ContextAction::ChangeCase(Case::Upper)),
                    ),
                    MenuItem::new(
                        "minúsculas",
                        keys::LOWER,
                        some(has_selection, ContextAction::ChangeCase(Case::Lower)),
                    ),
                    MenuItem::new(
                        "Tipo Título",
                        "",
                        some(has_selection, ContextAction::ChangeCase(Case::Title)),
                    ),
                ]);
                if language == Language::Markdown {
                    let label = if self.previews.contains(&id) {
                        "Ocultar vista previa"
                    } else {
                        "Vista previa de Markdown"
                    };
                    items.push(MenuItem::new(
                        label,
                        keys::PREVIEW,
                        Some(ContextAction::TogglePreview),
                    ));
                    items.push(MenuItem::new(
                        "Exportar a PDF…",
                        "",
                        Some(ContextAction::ExportPdf(id)),
                    ));
                }
                items.extend([
                    MenuItem::Separator,
                    MenuItem::new("Buscar…", keys::FIND, Some(ContextAction::FindSelection)),
                    MenuItem::new("Buscar todo", "", Some(ContextAction::FindAll)),
                    MenuItem::new("Marcar todo", "", Some(ContextAction::MarkAll)),
                    MenuItem::Separator,
                    MenuItem::new(
                        "Poner/quitar marcador",
                        keys::BOOKMARK,
                        Some(ContextAction::ToggleBookmark),
                    ),
                    MenuItem::new("Ir a línea…", keys::GOTO, Some(ContextAction::GoToLine)),
                ]);
                items
            }
            MenuTarget::Encoding(id) => {
                let Some(document) = self.workspace.document(id) else {
                    return Vec::new();
                };
                let current = document.format().encoding;
                let can_reopen = document.path().is_some() && !document.is_modified();
                let mut items = vec![MenuItem::Header("Guardar con la codificación".to_owned())];
                items.extend(TextEncoding::ALL.map(|encoding| {
                    MenuItem::check(
                        encoding.label(),
                        encoding == current,
                        Some(ContextAction::SaveWithEncoding(encoding)),
                    )
                }));
                items.push(MenuItem::Separator);
                items.push(MenuItem::Header(if can_reopen {
                    "Reabrir con la codificación".to_owned()
                } else if document.path().is_none() {
                    "Reabrir (no está guardado en disco)".to_owned()
                } else {
                    "Reabrir (guarda antes)".to_owned()
                }));
                items.extend(TextEncoding::ALL.map(|encoding| {
                    MenuItem::check(
                        encoding.label(),
                        false,
                        some(can_reopen, ContextAction::ReopenWithEncoding(encoding)),
                    )
                }));
                items
            }
            MenuTarget::LineEnding(id) => {
                let current = self.workspace.document(id).map(|d| d.format().line_ending);
                vec![
                    MenuItem::Header("Finales de línea al guardar".to_owned()),
                    MenuItem::check(
                        "LF (Unix, macOS)",
                        current == Some(LineEnding::Lf),
                        Some(ContextAction::SetLineEnding(LineEnding::Lf)),
                    ),
                    MenuItem::check(
                        "CRLF (Windows)",
                        current == Some(LineEnding::CrLf),
                        Some(ContextAction::SetLineEnding(LineEnding::CrLf)),
                    ),
                ]
            }
            MenuTarget::View => vec![
                MenuItem::Header("Mostrar".to_owned()),
                MenuItem::check(
                    "Espacios y tabuladores",
                    self.config.show_whitespace,
                    Some(ContextAction::ToggleWhitespace),
                ),
                MenuItem::check(
                    "Finales de línea",
                    self.config.show_line_endings,
                    Some(ContextAction::ToggleLineEndings),
                ),
                MenuItem::check(
                    "Guías de indentación",
                    self.config.indent_guides,
                    Some(ContextAction::ToggleIndentGuides),
                ),
            ],
            MenuTarget::Theme => {
                let mode = self.config.theme.unwrap_or_default();
                let mut items = vec![MenuItem::Header("Modo".to_owned())];
                for option in lorca_workspace::ThemeMode::ALL {
                    items.push(MenuItem::check(
                        option.label(),
                        option == mode,
                        Some(ContextAction::SetThemeMode(option)),
                    ));
                }
                for (title, appearance) in [
                    ("Temas oscuros", lorca_workspace::Appearance::Dark),
                    ("Temas claros", lorca_workspace::Appearance::Light),
                ] {
                    items.push(MenuItem::Separator);
                    items.push(MenuItem::Header(title.to_owned()));
                    let chosen = self.config.theme_name(appearance);
                    for (index, spec) in self.themes.iter().enumerate() {
                        if spec.appearance == appearance {
                            items.push(MenuItem::check(
                                &spec.name,
                                spec.name == chosen,
                                Some(ContextAction::SetTheme(index)),
                            ));
                        }
                    }
                }
                items
            }
            MenuTarget::Tab(id) => {
                let selected = self.workspace.selected();
                let tabs = self.workspace.tabs();
                let position = tabs.iter().position(|t| *t == id).unwrap_or(0);
                let has_path = self
                    .workspace
                    .document(id)
                    .is_some_and(|d| d.path().is_some());
                let mut items = vec![
                    MenuItem::new("Cerrar", keys::CLOSE, Some(ContextAction::Close(id))),
                    MenuItem::new(
                        "Cerrar las demás",
                        "",
                        some(tabs.len() > 1, ContextAction::CloseOthers(id)),
                    ),
                    MenuItem::new(
                        "Cerrar las de la derecha",
                        "",
                        some(position + 1 < tabs.len(), ContextAction::CloseRight(id)),
                    ),
                ];
                if selected.len() >= 2 {
                    items.push(MenuItem::new(
                        format!("Cerrar seleccionadas ({})", selected.len()),
                        "",
                        Some(ContextAction::CloseSelected),
                    ));
                    items.push(MenuItem::new(
                        format!("Guardar seleccionadas ({})", selected.len()),
                        "",
                        Some(ContextAction::SaveSelected),
                    ));
                }
                items.push(MenuItem::Separator);
                let split_label = match selected.len() {
                    0 | 1 => "Dividir pantalla (selecciona 2 a 4)".to_owned(),
                    n => format!("Dividir pantalla ({})", n.min(lorca_workspace::MAX_PANES)),
                };
                items.push(MenuItem::new(
                    split_label,
                    keys::SPLIT,
                    some(selected.len() >= 2, ContextAction::Split),
                ));
                items.push(MenuItem::new(
                    "Quitar división",
                    "",
                    some(self.workspace.is_split(), ContextAction::Unsplit),
                ));
                items.push(MenuItem::Separator);
                items.push(MenuItem::new(
                    "Copiar ruta",
                    "",
                    some(has_path, ContextAction::CopyPath(id)),
                ));
                items.push(MenuItem::new(
                    keys::REVEAL,
                    "",
                    some(has_path, ContextAction::Reveal(id)),
                ));
                items
            }
        }
    }

    pub(super) fn handle_context(&mut self, action: ContextAction) -> Task<Message> {
        self.context_menu = None;
        match action {
            ContextAction::Cut | ContextAction::Copy => {
                let Some(document) = self.workspace.active_mut() else {
                    return Task::none();
                };
                let Some(selected) = document.editor.selected_text() else {
                    return Task::none();
                };
                if action == ContextAction::Cut
                    && let Err(error) = document.editor.delete_selection()
                {
                    self.show_error(error.to_string());
                }
                iced::clipboard::write(selected)
            }
            ContextAction::Paste => iced::clipboard::read().map(Message::Paste),
            ContextAction::Delete => {
                if let Some(document) = self.workspace.active_mut()
                    && let Err(error) = document.editor.delete_selection()
                {
                    self.show_error(error.to_string());
                }
                Task::none()
            }
            ContextAction::SelectAll => {
                if let Some(document) = self.workspace.active_mut() {
                    document.editor.select_all();
                }
                Task::none()
            }
            ContextAction::Undo => self.update(Message::Undo),
            ContextAction::Redo => self.update(Message::Redo),
            ContextAction::FindSelection => self.open_find(find::PATTERN_ID),
            ContextAction::FindAll | ContextAction::MarkAll => {
                self.use_word_under_cursor_as_pattern();
                let message = if action == ContextAction::FindAll {
                    find::FindMessage::FindAll {
                        all_documents: false,
                    }
                } else {
                    find::FindMessage::MarkAll
                };
                self.find.open = true;
                self.handle_find(message)
            }
            ContextAction::ToggleBookmark => {
                self.bookmark_action(super::BookmarkAction::Toggle);
                Task::none()
            }
            ContextAction::GoToLine => self.open_goto(),
            ContextAction::Close(id) => self.request_close(id, Then::CloseTab),
            ContextAction::CloseOthers(id) => {
                let others: Vec<_> = self
                    .workspace
                    .tabs()
                    .iter()
                    .copied()
                    .filter(|t| *t != id)
                    .collect();
                self.close_many(others)
            }
            ContextAction::CloseRight(id) => {
                let tabs = self.workspace.tabs();
                let right = tabs
                    .iter()
                    .position(|t| *t == id)
                    .map(|index| tabs.iter().skip(index + 1).copied().collect())
                    .unwrap_or_default();
                self.close_many(right)
            }
            ContextAction::CloseSelected => {
                let selected = self.workspace.selected().to_vec();
                self.close_many(selected)
            }
            ContextAction::SaveSelected => {
                self.save_selected();
                Task::none()
            }
            ContextAction::CopyPath(id) => match self.workspace.document(id).and_then(|d| d.path())
            {
                Some(path) => iced::clipboard::write(path.display().to_string()),
                None => Task::none(),
            },
            ContextAction::Reveal(id) => {
                if let Some(path) = self.workspace.document(id).and_then(|d| d.path())
                    && let Err(error) = reveal_in_file_manager(path)
                {
                    self.show_error(format!("No se pudo mostrar el archivo: {error}"));
                }
                Task::none()
            }
            ContextAction::Split => {
                let selected = self.workspace.selected().to_vec();
                self.workspace.split(&selected);
                Task::none()
            }
            ContextAction::Unsplit => {
                self.workspace.unsplit();
                Task::none()
            }
            ContextAction::SetLineEnding(line_ending) => {
                if let Some(document) = self.workspace.active_mut() {
                    let format = document.format();
                    document.set_format(TextFormat {
                        line_ending,
                        ..format
                    });
                }
                Task::none()
            }
            ContextAction::SaveWithEncoding(encoding) => {
                if let Some(document) = self.workspace.active_mut() {
                    let format = document.format();
                    document.set_format(TextFormat { encoding, ..format });
                }
                Task::none()
            }
            ContextAction::ReopenWithEncoding(encoding) => {
                if let Some(id) = self.workspace.active_id() {
                    match self.workspace.reload_with_encoding(id, encoding) {
                        Ok(()) => {
                            // El texto es otro: se descarta todo lo calculado sobre él.
                            self.highlights.remove(&id);
                            self.mark_cache.remove(&id);
                            self.preview_cache.remove(&id);
                            self.find_cache = None;
                            self.show_info(format!("Reabierto como {}", encoding.label()));
                        }
                        Err(error) => self.show_error(error.to_string()),
                    }
                }
                Task::none()
            }
            ContextAction::GoToDefinition => {
                self.request_definition();
                Task::none()
            }
            ContextAction::ShowHover => {
                self.request_hover();
                Task::none()
            }
            ContextAction::SetThemeMode(mode) => {
                self.config.theme = Some(mode);
                self.save_config();
                Task::none()
            }
            ContextAction::SetTheme(index) => {
                self.select_theme(index);
                Task::none()
            }
            ContextAction::ToggleWhitespace => {
                self.config.show_whitespace = !self.config.show_whitespace;
                self.save_config();
                Task::none()
            }
            ContextAction::ToggleLineEndings => {
                self.config.show_line_endings = !self.config.show_line_endings;
                self.save_config();
                Task::none()
            }
            ContextAction::ToggleIndentGuides => {
                self.config.indent_guides = !self.config.indent_guides;
                self.save_config();
                Task::none()
            }
            ContextAction::FormatDocument => {
                self.format_document(false);
                Task::none()
            }
            ContextAction::MinifyJson => {
                self.format_document(true);
                Task::none()
            }
            ContextAction::ChangeCase(case) => {
                self.change_selection_case(case);
                Task::none()
            }
            ContextAction::TogglePreview => {
                self.toggle_preview();
                Task::none()
            }
            ContextAction::ExportPdf(id) => self.update(Message::ExportPdf(id)),
        }
    }

    /// Formatea (o minifica) el JSON/SQL de la selección o de todo el documento.
    fn format_document(&mut self, minify: bool) {
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let language = document.language();
        let selection = document.editor.selection();
        let (range, source) = if selection.is_empty() {
            (
                0..document.editor.buffer().len_chars(),
                document.editor.text(),
            )
        } else {
            (
                selection.range(),
                document.editor.selected_text().unwrap_or_default(),
            )
        };
        let result = match (language, minify) {
            (Language::Json, true) => minify_json(&source),
            (Language::Json, false) => format_json(&source, &self.config.indent_unit()),
            (Language::Sql, false) => Ok(format_sql(&source)),
            _ => {
                self.show_info("Formatear está disponible para JSON y SQL".to_owned());
                return;
            }
        };
        match result {
            Ok(formatted) if formatted == source => {
                self.show_info("Ya estaba formateado".to_owned());
            }
            Ok(formatted) => {
                if let Err(error) = document.editor.replace_ranges(&[(range, formatted)]) {
                    self.show_error(error.to_string());
                }
            }
            Err(error) => {
                // Lleva el cursor al punto del error.
                let offset = Buffer::from_text(&source).position_to_char(Position::new(
                    error.line.saturating_sub(1),
                    error.column.saturating_sub(1),
                ));
                document.editor.set_cursor(range.start + offset, false);
                self.show_error(error.to_string());
            }
        }
    }

    /// Cambia mayúsculas/minúsculas de la selección y la mantiene seleccionada.
    fn change_selection_case(&mut self, case: Case) {
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        let Some(selected) = document.editor.selected_text() else {
            self.show_info("Selecciona primero el texto".to_owned());
            return;
        };
        let range = document.editor.selection().range();
        let converted = change_case(&selected, case);
        let length = converted.chars().count();
        if let Err(error) = document
            .editor
            .replace_ranges(&[(range.clone(), converted)])
        {
            self.show_error(error.to_string());
            return;
        }
        document
            .editor
            .set_selection(Selection::new(range.start, range.start + length));
    }

    pub(super) fn toggle_preview(&mut self) {
        let Some(document) = self.workspace.active() else {
            return;
        };
        let id = document.id();
        if self.previews.remove(&id) {
            return;
        }
        if document.language() == Language::Markdown {
            self.previews.insert(id);
        } else {
            self.show_info("La vista previa es para documentos Markdown".to_owned());
        }
    }

    /// Si no hay selección, toma la palabra bajo el cursor como texto a buscar.
    fn use_word_under_cursor_as_pattern(&mut self) {
        let Some(document) = self.workspace.active_mut() else {
            return;
        };
        if let Some(word) = super::find_actions::select_word_near_cursor(&mut document.editor) {
            self.find.pattern = word;
        }
    }

    /// Cierra varias pestañas, preguntando por las que tengan cambios.
    fn close_many(&mut self, ids: Vec<DocumentId>) -> Task<Message> {
        self.close_queue = ids.into();
        self.process_close_queue()
    }

    pub(super) fn process_close_queue(&mut self) -> Task<Message> {
        match self.close_queue.pop_front() {
            Some(id) => self.request_close(id, Then::CloseAndContinue),
            None => Task::none(),
        }
    }

    /// Guarda las pestañas seleccionadas que ya tienen ruta.
    fn save_selected(&mut self) {
        let selected = self.workspace.selected().to_vec();
        let (mut saved, mut untitled, mut errors) = (0, 0, Vec::new());
        for id in selected {
            match self.workspace.document(id) {
                Some(document) if document.path().is_none() => untitled += 1,
                Some(document) if !document.is_modified() => {}
                Some(_) => match self.workspace.save(id) {
                    Ok(()) => saved += 1,
                    Err(error) => errors.push(error.to_string()),
                },
                None => {}
            }
        }
        if !errors.is_empty() {
            self.show_error(errors.join(" · "));
        } else if untitled > 0 {
            self.show_info(format!(
                "Guardados {saved}; {untitled} sin título necesitan «Guardar como»"
            ));
        } else {
            self.show_info(format!("Guardados {saved}"));
        }
    }
}

/// Muestra el archivo en Finder, el Explorador o el gestor de archivos.
fn reveal_in_file_manager(path: &Path) -> std::io::Result<()> {
    use std::process::Command;
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        command
    } else if cfg!(windows) {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,{}", path.display()));
        command
    } else {
        let mut command = Command::new("xdg-open");
        command.arg(path.parent().unwrap_or(path));
        command
    };
    command.spawn().map(|_| ())
}

/// Abre un enlace de la vista previa con la aplicación predeterminada del sistema.
pub fn open_url(url: &str) -> std::io::Result<()> {
    use std::process::Command;
    let mut command = if cfg!(target_os = "macos") {
        Command::new("open")
    } else if cfg!(windows) {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", ""]);
        command
    } else {
        Command::new("xdg-open")
    };
    command.arg(url).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id_list(ws: &mut lorca_workspace::Workspace, n: usize) -> Vec<DocumentId> {
        (0..n).map(|_| ws.new_document()).collect()
    }

    #[test]
    fn layout_has_one_pane_per_document_in_order() {
        let mut ws = lorca_workspace::Workspace::new();
        let ids = id_list(&mut ws, 4);
        let layout = PaneLayout::new(&ids).unwrap();
        let shown: Vec<_> = layout
            .order
            .iter()
            .map(|p| *layout.state.get(*p).unwrap())
            .collect();
        assert_eq!(shown, ids);
        assert!(PaneLayout::new(&[]).is_none());
    }
}
