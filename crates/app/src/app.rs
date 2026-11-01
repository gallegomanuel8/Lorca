//! Estado de la aplicación, mensajes y vista principal.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use iced::theme::Mode;
use iced::time::{Duration, Instant};
use iced::widget::{
    button, column, container, mouse_area, pane_grid, pick_list, row, scrollable, space, text,
    tooltip,
};
use iced::{
    Alignment, Element, Length, Point, Size, Subscription, Task, Theme, event, keyboard, system,
    window,
};
use lorca_formats::{PdfFonts, markdown_to_pdf};
use lorca_search::SearchQuery;
use lorca_syntax::{Highlights, Language, SyntaxError};
use lorca_workspace::{
    Config, DiskStamp, Document, DocumentId, SessionStore, THEMES_DIR, ThemeMode, ThemeSet,
    Workspace, data_dir,
};

use crate::context_menu::{self, ContextAction, ContextMenu, MenuTarget, click_area, context_area};
use crate::dialogs::{self, SaveChoice};
use crate::editor::{DocView, EditorAction, Metrics, apply_action};
use crate::find::{self, FindMessage, FindState, SearchResults};
use crate::menu;
use crate::palette::{self, Palette};
use crate::shortcuts::shortcut;
use crate::theme::{ActiveTheme, system_is_dark};
use crate::watcher;
use crate::welcome;

#[path = "find_actions.rs"]
mod find_actions;

pub use find_actions::BookmarkAction;
use find_actions::{Focus, MatchCache};

#[path = "panes.rs"]
mod panes;

#[path = "lsp_actions.rs"]
mod lsp_actions;

use lsp_actions::LspState;

use panes::PaneLayout;

/// Tamaño inicial de la ventana.
pub const WINDOW_SIZE: Size = Size::new(1100.0, 720.0);

/// Pausa sin cambios tras la que se guarda la sesión.
///
/// Es corta porque en macOS ⌘Q (menú por defecto de winit) termina el proceso sin avisar
/// a la aplicación: lo no guardado en la sesión en ese momento se perdería.
const SESSION_IDLE: Duration = Duration::from_millis(300);
/// Tiempo máximo con cambios sin guardar en la sesión aunque se siga tecleando.
const SESSION_MAX_DELAY: Duration = Duration::from_secs(2);
/// Frecuencia con la que se comprueba si toca guardar la sesión.
const SESSION_TICK: Duration = Duration::from_millis(100);

/// Qué hacer con un documento cuando termina el flujo de guardado o confirmación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Solo guardar.
    Nothing,
    /// Cerrar la pestaña.
    CloseTab,
    /// Seguir con el siguiente documento antes de salir.
    ContinueQuit,
    /// Cerrar la pestaña y seguir con la siguiente de `close_queue`.
    CloseAndContinue,
}

#[derive(Debug, Clone)]
pub enum Message {
    Editor(DocumentId, EditorAction),
    NewDocument,
    Open,
    FilesPicked(Vec<PathBuf>),
    Save,
    SaveAs,
    SavePathPicked {
        id: DocumentId,
        path: Option<PathBuf>,
        then: Then,
    },
    CloseActive,
    CloseTab(DocumentId),
    SaveChoiceMade {
        id: DocumentId,
        choice: SaveChoice,
        then: Then,
    },
    /// Clic en una pestaña (con Cmd/Ctrl o Mayús, selecciona).
    TabPressed(DocumentId),
    TabContextMenu(DocumentId, Point),
    ActivateTab(DocumentId),
    FindInFolder,
    FolderPicked(Option<PathBuf>),
    FolderSearched {
        pattern: String,
        result: lorca_search::FolderSearch,
    },
    OpenPalette,
    PaletteInput(String),
    PaletteMove(i32),
    /// Ejecuta la orden indicada (o la resaltada si es `None`).
    PaletteRun(Option<usize>),
    ClosePalette,
    /// El ratón entra en una pestaña (resaltado y arrastre).
    TabHovered(DocumentId),
    TabUnhovered(DocumentId),
    /// Botón izquierdo soltado en cualquier sitio: termina el arrastre de pestañas.
    MouseReleased,
    Context(ContextAction),
    CloseContextMenu,
    Paste(Option<String>),
    /// Abre el menú de la barra de estado («UTF-8», «LF», «¶»).
    StatusMenu(MenuTarget, Point),
    OpenUrl(String),
    /// Otro programa cambió (o borró) un archivo abierto.
    FileChanged(PathBuf),
    /// Respuesta a «¿recargar?» para un documento con cambios sin guardar.
    ExternalChangeChoice {
        id: DocumentId,
        reload: bool,
    },
    /// Exportar la vista previa de un Markdown a PDF: pedir ruta, generar y avisar.
    ExportPdf(DocumentId),
    /// Exportar el documento activo (desde el menú).
    ExportActivePdf,
    /// Instala la barra de menús nativa (tras arrancar, en el hilo principal).
    InstallMenu,
    ExportPdfPath(DocumentId, Option<PathBuf>),
    PdfExported(Result<PathBuf, String>),
    PaneClicked(pane_grid::Pane),
    PaneResized(pane_grid::ResizeEvent),
    ToggleSplit,
    WindowResized(Size),
    NextTab,
    PreviousTab,
    Undo,
    Redo,
    CloseRequested,
    DismissNotice,
    /// El modo claro/oscuro del sistema ha cambiado (o se ha consultado al arrancar).
    SystemTheme(Mode),
    ThemeSelected(ThemeMode),
    /// Cambia el tamaño de letra: +1, −1 o 0 para volver al de por defecto.
    Zoom(i8),
    /// Abre `config.toml` en una pestaña.
    OpenConfig,
    /// Crea una copia editable del tema actual y la abre.
    NewTheme,
    /// La suscripción de los servidores de lenguaje está lista para recibir eventos.
    LspConnected(iced::futures::channel::mpsc::UnboundedSender<lorca_lsp::Event>),
    Lsp(lorca_lsp::Event),
    /// Clic en una propuesta del desplegable (posición en la lista visible).
    CompletionPick(usize),
    TriggerCompletion,
    ShowHover,
    GoToDefinition,
    /// Siguiente (`true`) o anterior problema del documento.
    NextProblem(bool),
    LspRestart,
    /// Opción marcada en la pantalla de bienvenida (aún sin confirmar).
    WelcomeChoose(ThemeMode),
    WelcomeDone,
    /// Latido para el guardado diferido de la sesión.
    Tick,
    LanguageSelected(Language),
    /// Resultado del resaltado en segundo plano de una revisión de un documento.
    Highlighted {
        id: DocumentId,
        revision: u64,
        language: Language,
        result: Result<Option<Arc<Highlights>>, String>,
    },
    Find(FindMessage),
    OpenFind,
    OpenReplace,
    FindNext,
    FindPrevious,
    FindWordUnderCursor,
    GoToLine,
    Escape,
    Bookmark(BookmarkAction),
    ModifiersChanged(keyboard::Modifiers),
}

/// Resaltado de un documento y estado de su cálculo en segundo plano.
#[derive(Debug, Default)]
struct HighlightState {
    /// Revisión y lenguaje del resaltado actual.
    done: Option<(u64, Language)>,
    highlights: Option<Arc<Highlights>>,
    /// Hay un cálculo en marcha; no se lanza otro hasta que termine.
    pending: bool,
}

/// Mensaje informativo o de error que se muestra en la barra de estado.
#[derive(Debug, Clone)]
struct Notice {
    text: String,
    is_error: bool,
}

pub struct Lorca {
    workspace: Workspace,
    views: HashMap<DocumentId, DocView>,
    notice: Option<Notice>,
    /// Hay un diálogo nativo abierto: se ignoran las órdenes que abrirían otro.
    dialog_open: bool,
    /// Documentos con cambios que quedan por confirmar antes de salir. Solo se usa si
    /// no se puede guardar la sesión.
    quit_queue: Option<VecDeque<DocumentId>>,
    config: Config,
    /// Directorio de configuración y sesión (`None` si el sistema no ofrece ninguno).
    data_dir: Option<PathBuf>,
    session: Option<SessionStore>,
    /// Primer y último cambio aún no guardados en la sesión.
    session_dirty: Option<(Instant, Instant)>,
    system_mode: Mode,
    /// Opción marcada en la pantalla de bienvenida; `Some` mientras se muestra.
    welcome_choice: Option<ThemeMode>,
    /// Temas disponibles (incluidos y del usuario) y el que se está mostrando.
    themes: ThemeSet,
    active_theme: ActiveTheme,
    highlights: HashMap<DocumentId, HighlightState>,
    find: FindState,
    /// Texto de «Ir a línea»; `Some` mientras la barra está abierta.
    goto: Option<String>,
    focus: Focus,
    results: Option<SearchResults>,
    /// Consultas marcadas con «Marcar todo» en cada documento.
    marks: HashMap<DocumentId, SearchQuery>,
    find_cache: Option<MatchCache>,
    mark_cache: HashMap<DocumentId, MatchCache>,
    modifiers: keyboard::Modifiers,
    context_menu: Option<ContextMenu>,
    /// Pestañas pendientes de cerrar (Cerrar las demás, seleccionadas…).
    close_queue: VecDeque<DocumentId>,
    pane_layout: Option<PaneLayout>,
    /// Documentos Markdown con vista previa y su contenido analizado por revisión.
    previews: HashSet<DocumentId>,
    preview_cache: HashMap<DocumentId, (u64, iced::widget::markdown::Content)>,
    window_size: Size,
    hovered_tab: Option<DocumentId>,
    /// Pestaña que se está arrastrando para reordenarla.
    dragging_tab: Option<DocumentId>,
    palette: Option<Palette>,
    /// Barra de menús nativa; se conserva para que siga viva.
    native_menu: Option<menu::NativeMenu>,
    lsp: LspState,
}

impl Lorca {
    /// Estado inicial: carga la configuración, restaura la sesión anterior y abre los
    /// archivos pasados por línea de órdenes.
    pub fn boot() -> (Self, Task<Message>) {
        let data_dir = data_dir();
        let mut problems = Vec::new();

        let config = match &data_dir {
            Some(dir) => Config::load(&dir.join(Config::FILE_NAME)).unwrap_or_else(|error| {
                problems.push(error.to_string());
                Config::default()
            }),
            None => {
                problems.push(
                    "No se encontró el directorio de configuración: la sesión no se guardará"
                        .to_owned(),
                );
                Config::default()
            }
        };

        let themes = match &data_dir {
            Some(dir) => {
                let (themes, errors) = ThemeSet::load(&dir.join(THEMES_DIR));
                problems.extend(errors.iter().map(ToString::to_string));
                themes
            }
            None => ThemeSet::builtin(),
        };
        let appearance = config.theme.unwrap_or_default().appearance(None);
        let active_theme = ActiveTheme::new(&themes, config.theme_name(appearance), appearance);

        let mut workspace = Workspace::new();
        let mut views = HashMap::new();
        let session = data_dir.clone().map(SessionStore::new);
        if let Some(store) = &session {
            match store.load() {
                Ok(Some(saved)) => {
                    let report = workspace.restore(&saved, |name| store.read_backup(name));
                    for (id, scroll_x, scroll_y) in report.scrolls {
                        views.insert(
                            id,
                            DocView {
                                scroll_x,
                                scroll_y,
                                ..DocView::default()
                            },
                        );
                    }
                    problems.extend(report.warnings);
                }
                Ok(None) => {}
                Err(error) => problems.push(error.to_string()),
            }
        }

        let mut app = Self {
            workspace,
            views,
            notice: None,
            dialog_open: false,
            quit_queue: None,
            welcome_choice: config.theme.is_none().then_some(ThemeMode::Auto),
            themes,
            active_theme,
            config,
            data_dir,
            session,
            session_dirty: None,
            system_mode: Mode::None,
            highlights: HashMap::new(),
            find: FindState::default(),
            goto: None,
            focus: Focus::Editor,
            results: None,
            marks: HashMap::new(),
            find_cache: None,
            mark_cache: HashMap::new(),
            modifiers: keyboard::Modifiers::default(),
            context_menu: None,
            close_queue: VecDeque::new(),
            pane_layout: None,
            previews: HashSet::new(),
            preview_cache: HashMap::new(),
            window_size: WINDOW_SIZE,
            hovered_tab: None,
            dragging_tab: None,
            palette: None,
            native_menu: None,
            lsp: LspState::default(),
        };
        if !problems.is_empty() {
            app.show_error(problems.join(" · "));
        }
        let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
        app.open_paths(&paths);
        app.ensure_document();
        app.sync_panes();
        app.refresh_theme();
        let highlight = app.schedule_highlight();
        (
            app,
            Task::batch([
                system::theme().map(Message::SystemTheme),
                highlight,
                Task::done(Message::InstallMenu),
            ]),
        )
    }

    pub fn title(&self) -> String {
        match self.workspace.active() {
            Some(doc) => {
                let modified = if doc.is_modified() { " ●" } else { "" };
                format!("{}{modified} — Lorca", doc.display_name())
            }
            None => "Lorca".to_owned(),
        }
    }

    pub fn theme(&self) -> Theme {
        self.active_theme.iced.clone()
    }

    /// Medidas del texto del editor según la configuración.
    pub(crate) fn metrics(&self) -> Metrics {
        Metrics::new(f32::from(self.config.font_size()), self.config.tab_width())
    }

    /// Aspecto (claro u oscuro) que toca mostrar ahora.
    fn appearance(&self) -> lorca_workspace::Appearance {
        self.welcome_choice
            .or(self.config.theme)
            .unwrap_or_default()
            .appearance(system_is_dark(self.system_mode))
    }

    /// Reconstruye el tema activo si cambió el modo, el sistema o el tema elegido.
    fn refresh_theme(&mut self) {
        let appearance = self.appearance();
        let name = self.config.theme_name(appearance);
        if !self.active_theme.is(name, appearance) {
            self.active_theme = ActiveTheme::new(&self.themes, name, appearance);
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            event::listen_with(shortcut),
            window::close_requests().map(|_| Message::CloseRequested),
            system::theme_changes().map(Message::SystemTheme),
            menu::events(),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
            watcher::watch(self.watched()),
            crate::lsp_view::events(),
        ];
        if self.palette.is_some() {
            subscriptions.push(event::listen_with(palette_keys));
        }
        if self.session_dirty.is_some() {
            subscriptions.push(iced::time::every(SESSION_TICK).map(|_| Message::Tick));
        }
        Subscription::batch(subscriptions)
    }

    /// Archivos abiertos, `config.toml` y la carpeta de temas.
    fn watched(&self) -> watcher::Watched {
        let mut files: Vec<PathBuf> = self
            .workspace
            .documents()
            .filter_map(|d| d.path().map(PathBuf::from))
            .collect();
        let mut folders = Vec::new();
        if let Some(dir) = &self.data_dir {
            files.push(dir.join(Config::FILE_NAME));
            let themes = dir.join(THEMES_DIR);
            if themes.is_dir() {
                folders.push(themes);
            }
        }
        watcher::Watched { files, folders }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.sync_panes();
        self.refresh_search();
        self.refresh_previews();
        self.refresh_theme();
        self.sync_lsp();
        self.refresh_diagnostics();
        Task::batch([task, self.schedule_highlight()])
    }

    fn handle(&mut self, message: Message) -> Task<Message> {
        if affects_session(&message) {
            let now = Instant::now();
            let first = self.session_dirty.map_or(now, |(first, _)| first);
            self.session_dirty = Some((first, now));
        }

        match message {
            Message::Editor(id, action) => {
                match action {
                    EditorAction::ContextMenu { position, .. } => {
                        self.open_context_menu(position, MenuTarget::Text(id));
                    }
                    EditorAction::CursorPoint(point) => {
                        self.lsp.cursor_point = Some((id, point));
                        return Task::none();
                    }
                    EditorAction::Completion(key) => {
                        self.completion_key(key);
                        return Task::none();
                    }
                    _ => {}
                }
                self.apply_editor_action(id, action.clone());
                self.after_editor_action(id, &action);
                if matches!(action, EditorAction::GoToDefinition { .. }) {
                    self.request_definition();
                }
                Task::none()
            }
            Message::NewDocument => {
                self.workspace.new_document();
                Task::none()
            }
            Message::Open => {
                if self.dialog_open {
                    return Task::none();
                }
                self.dialog_open = true;
                Task::perform(dialogs::pick_files_to_open(), Message::FilesPicked)
            }
            Message::FilesPicked(paths) => {
                self.dialog_open = false;
                self.open_paths(&paths);
                Task::none()
            }
            Message::Save => match self.workspace.active_id() {
                Some(id) => self.save(id, Then::Nothing),
                None => Task::none(),
            },
            Message::SaveAs => match self.workspace.active_id() {
                Some(id) => self.save_as(id, Then::Nothing),
                None => Task::none(),
            },
            Message::SavePathPicked { id, path, then } => {
                self.dialog_open = false;
                let Some(path) = path else {
                    self.quit_queue = None;
                    self.close_queue.clear();
                    return Task::none();
                };
                match self.workspace.save_as(id, &path) {
                    Ok(()) => {
                        self.notify_saved(id);
                        self.finish(id, then)
                    }
                    Err(error) => self.abort_with_error(error.to_string()),
                }
            }
            Message::CloseActive => match self.workspace.active_id() {
                Some(id) => self.request_close(id, Then::CloseTab),
                None => Task::none(),
            },
            Message::CloseTab(id) => self.request_close(id, Then::CloseTab),
            Message::SaveChoiceMade { id, choice, then } => {
                self.dialog_open = false;
                match choice {
                    SaveChoice::Save => self.save(id, then),
                    SaveChoice::Discard => self.finish(id, then),
                    SaveChoice::Cancel => {
                        self.quit_queue = None;
                        self.close_queue.clear();
                        Task::none()
                    }
                }
            }
            Message::TabPressed(id) => {
                self.tab_pressed(id);
                self.dragging_tab = Some(id);
                Task::none()
            }
            Message::FindInFolder => self.find_in_folder(),
            Message::FolderPicked(folder) => {
                self.dialog_open = false;
                match folder {
                    Some(root) => self.search_in_folder(root),
                    None => Task::none(),
                }
            }
            Message::FolderSearched { pattern, result } => {
                self.show_folder_results(pattern, result);
                Task::none()
            }
            Message::ActivateTab(id) => {
                self.workspace.clear_selection();
                self.workspace.activate(id);
                Task::none()
            }
            Message::OpenPalette => {
                self.context_menu = None;
                self.palette = Some(Palette::default());
                iced::widget::operation::focus(palette::INPUT_ID)
            }
            Message::PaletteInput(query) => {
                if let Some(palette) = &mut self.palette {
                    palette.query = query;
                    palette.selected = 0;
                }
                Task::none()
            }
            Message::PaletteMove(delta) => {
                let count = self.palette_matches().len();
                if let Some(palette) = &mut self.palette {
                    palette.selected = palette::move_selection(palette.selected, delta, count);
                }
                Task::none()
            }
            Message::PaletteRun(index) => {
                let Some(selected) = self.palette.as_ref().map(|p| p.selected) else {
                    return Task::none();
                };
                let command = self
                    .palette_matches()
                    .into_iter()
                    .nth(index.unwrap_or(selected));
                self.palette = None;
                match command {
                    Some(command) => self.update(command.message),
                    None => Task::none(),
                }
            }
            Message::ClosePalette => {
                self.palette = None;
                Task::none()
            }
            Message::TabHovered(id) => {
                self.hovered_tab = Some(id);
                if let Some(dragged) = self.dragging_tab
                    && dragged != id
                    && let Some(index) = self.workspace.tabs().iter().position(|t| *t == id)
                {
                    self.workspace.move_tab(dragged, index);
                }
                Task::none()
            }
            Message::TabUnhovered(id) => {
                if self.hovered_tab == Some(id) {
                    self.hovered_tab = None;
                }
                Task::none()
            }
            Message::MouseReleased => {
                self.dragging_tab = None;
                Task::none()
            }
            Message::TabContextMenu(id, position) => {
                self.open_context_menu(position, MenuTarget::Tab(id));
                Task::none()
            }
            Message::Context(action) => self.handle_context(action),
            Message::CloseContextMenu => {
                self.context_menu = None;
                Task::none()
            }
            Message::StatusMenu(target, position) => {
                self.context_menu = Some(ContextMenu { position, target });
                Task::none()
            }
            Message::InstallMenu => {
                match menu::install() {
                    Ok(installed) => self.native_menu = Some(installed),
                    Err(error) => self.show_error(format!("No se pudo crear el menú: {error}")),
                }
                Task::none()
            }
            Message::ExportActivePdf => match self.workspace.active_id() {
                Some(id) => self.update(Message::ExportPdf(id)),
                None => Task::none(),
            },
            Message::ExportPdf(id) => {
                if self.dialog_open {
                    return Task::none();
                }
                let Some(document) = self.workspace.document(id) else {
                    return Task::none();
                };
                let name = document.display_name();
                let stem = name
                    .rsplit_once('.')
                    .map_or(name.as_str(), |(stem, _)| stem);
                let directory = document.path().and_then(|p| p.parent()).map(PathBuf::from);
                self.dialog_open = true;
                Task::perform(
                    dialogs::pick_pdf_path(format!("{stem}.pdf"), directory),
                    move |path| Message::ExportPdfPath(id, path),
                )
            }
            Message::ExportPdfPath(id, path) => {
                self.dialog_open = false;
                let (Some(path), Some(document)) = (path, self.workspace.document(id)) else {
                    return Task::none();
                };
                let text = document.editor.text();
                let title = document.display_name();
                self.show_info("Generando el PDF…".to_owned());
                Task::perform(
                    async move {
                        let fonts = PdfFonts::system().map_err(|e| e.to_string())?;
                        let pdf =
                            markdown_to_pdf(&text, &title, &fonts).map_err(|e| e.to_string())?;
                        std::fs::write(&path, pdf).map_err(|e| e.to_string())?;
                        Ok(path)
                    },
                    Message::PdfExported,
                )
            }
            Message::PdfExported(result) => {
                match result {
                    Ok(path) => self.show_info(format!("PDF guardado en «{}»", path.display())),
                    Err(error) => self.show_error(format!("No se pudo exportar el PDF: {error}")),
                }
                Task::none()
            }
            Message::FileChanged(path) => self.file_changed(&path),
            Message::ExternalChangeChoice { id, reload } => {
                self.dialog_open = false;
                if reload {
                    self.reload_document(id);
                } else if let Some(document) = self.workspace.document_mut(id) {
                    document.acknowledge_disk_change();
                }
                Task::none()
            }
            Message::OpenUrl(url) => {
                if let Err(error) = panes::open_url(&url) {
                    self.show_error(format!("No se pudo abrir «{url}»: {error}"));
                }
                Task::none()
            }
            Message::Paste(contents) => {
                if let Some(contents) = contents
                    && let Some(document) = self.workspace.active_mut()
                    && let Err(error) = document.editor.insert_text(&contents)
                {
                    self.show_error(error.to_string());
                }
                Task::none()
            }
            Message::PaneClicked(pane) => {
                let id = self
                    .pane_layout
                    .as_ref()
                    .and_then(|layout| layout.state.get(pane))
                    .copied();
                if let Some(id) = id {
                    self.workspace.activate(id);
                    self.focus = Focus::Editor;
                }
                Task::none()
            }
            Message::PaneResized(event) => {
                if let Some(layout) = &mut self.pane_layout {
                    layout.state.resize(event.split, event.ratio);
                }
                Task::none()
            }
            Message::ToggleSplit => {
                self.toggle_split();
                Task::none()
            }
            Message::WindowResized(size) => {
                self.window_size = size;
                Task::none()
            }
            Message::NextTab => {
                self.workspace.activate_next();
                Task::none()
            }
            Message::PreviousTab => {
                self.workspace.activate_previous();
                Task::none()
            }
            Message::Undo => {
                self.undo_redo(true);
                Task::none()
            }
            Message::Redo => {
                self.undo_redo(false);
                Task::none()
            }
            Message::CloseRequested => {
                if self.dialog_open {
                    return Task::none();
                }
                // Con la sesión guardada no se pierde nada: se sale sin preguntar. Si no se
                // puede guardar, se vuelve a preguntar documento por documento.
                if self.persist_session().is_ok() {
                    return iced::exit();
                }
                self.quit_queue = Some(self.workspace.modified_documents().into());
                self.process_quit()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                Task::none()
            }
            Message::ThemeSelected(mode) => {
                self.config.theme = Some(mode);
                self.save_config();
                Task::none()
            }
            Message::Zoom(delta) => {
                self.config.font_size = match delta {
                    0 => Config::DEFAULT_FONT_SIZE,
                    _ => self
                        .config
                        .font_size()
                        .saturating_add_signed(i16::from(delta))
                        .clamp(*Config::FONT_SIZES.start(), *Config::FONT_SIZES.end()),
                };
                self.save_config();
                self.show_info(format!("Tamaño de letra: {}", self.config.font_size));
                Task::none()
            }
            Message::OpenConfig => {
                let Some(dir) = self.data_dir.clone() else {
                    return Task::none();
                };
                let path = dir.join(Config::FILE_NAME);
                if !path.exists() {
                    self.save_config();
                }
                self.open_paths(&[path]);
                Task::none()
            }
            Message::NewTheme => {
                let Some(dir) = self.data_dir.clone() else {
                    return Task::none();
                };
                let current = self.active_theme.name();
                match self.themes.create_personal(&current, &dir.join(THEMES_DIR)) {
                    Ok((name, path)) => {
                        let appearance = self.active_theme.appearance;
                        match appearance {
                            lorca_workspace::Appearance::Light => self.config.light_theme = name,
                            lorca_workspace::Appearance::Dark => self.config.dark_theme = name,
                        }
                        self.save_config();
                        self.reload_themes();
                        self.open_paths(&[path]);
                        self.show_info(
                            "Tema creado: edítalo y guarda para ver los cambios".to_owned(),
                        );
                    }
                    Err(error) => self.show_error(error.to_string()),
                }
                Task::none()
            }
            Message::WelcomeChoose(mode) => {
                self.welcome_choice = Some(mode);
                Task::none()
            }
            Message::WelcomeDone => {
                self.config.theme = Some(self.welcome_choice.take().unwrap_or_default());
                self.save_config();
                Task::none()
            }
            Message::LanguageSelected(language) => {
                if let Some(document) = self.workspace.active_mut() {
                    document.set_language(language);
                }
                Task::none()
            }
            Message::Highlighted {
                id,
                revision,
                language,
                result,
            } => {
                let state = self.highlights.entry(id).or_default();
                let language_changed = state.done.map(|(_, l)| l) != Some(language);
                state.pending = false;
                state.done = Some((revision, language));
                match result {
                    Ok(highlights) => state.highlights = highlights,
                    Err(error) => {
                        state.highlights = None;
                        // Solo se avisa una vez por lenguaje, no en cada pulsación.
                        if language_changed {
                            self.show_error(error);
                        }
                    }
                }
                Task::none()
            }
            Message::Find(message) => self.handle_find(message),
            Message::OpenFind => self.open_find(find::PATTERN_ID),
            Message::OpenReplace => self.open_find(find::REPLACEMENT_ID),
            Message::FindNext => self.find_next(true),
            Message::FindPrevious => self.find_next(false),
            Message::FindWordUnderCursor => {
                self.find_word_under_cursor();
                Task::none()
            }
            Message::GoToLine => self.open_goto(),
            Message::Escape => {
                let had_popup =
                    self.lsp.completion.take().is_some() | self.lsp.hover.take().is_some();
                if !had_popup && self.palette.take().is_none() && self.context_menu.take().is_none()
                {
                    self.escape();
                }
                Task::none()
            }
            Message::LspConnected(sender) => {
                self.lsp_connected(sender);
                Task::none()
            }
            Message::Lsp(event) => {
                self.handle_lsp_event(event);
                Task::none()
            }
            Message::CompletionPick(row) => {
                self.accept_completion(row);
                Task::none()
            }
            Message::TriggerCompletion => {
                self.request_completion(None, true);
                Task::none()
            }
            Message::ShowHover => {
                self.context_menu = None;
                self.request_hover();
                Task::none()
            }
            Message::GoToDefinition => {
                self.context_menu = None;
                self.request_definition();
                Task::none()
            }
            Message::NextProblem(forward) => {
                self.jump_to_diagnostic(forward);
                Task::none()
            }
            Message::LspRestart => {
                self.restart_lsp();
                Task::none()
            }
            Message::Bookmark(action) => {
                self.bookmark_action(action);
                Task::none()
            }
            Message::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers;
                Task::none()
            }
            Message::Tick => {
                if let Some((first, last)) = self.session_dirty {
                    let now = Instant::now();
                    if now - last >= SESSION_IDLE || now - first >= SESSION_MAX_DELAY {
                        // El error ya se muestra en la barra de estado.
                        let _ = self.persist_session();
                    }
                }
                Task::none()
            }
        }
    }

    // --- Sesión y configuración -------------------------------------------------------

    /// Guarda la sesión ahora. En caso de error lo muestra y lo devuelve.
    fn persist_session(&mut self) -> Result<(), ()> {
        self.session_dirty = None;
        let Some(store) = self.session.as_mut() else {
            self.show_error("No se puede guardar la sesión: no hay directorio de datos".to_owned());
            return Err(());
        };
        let views = &self.views;
        let result = store.save(&self.workspace, |id| {
            views
                .get(&id)
                .map_or((0.0, 0.0), |v| (v.scroll_x, v.scroll_y))
        });
        result.map_err(|error| self.show_error(format!("No se pudo guardar la sesión: {error}")))
    }

    /// Vuelve a leer `config.toml` (lo ha cambiado el usuario u otro programa). Si tiene
    /// errores se avisa y se mantiene la configuración actual.
    fn reload_config(&mut self) {
        let Some(dir) = &self.data_dir else { return };
        let previous_lsp = self.config.lsp.clone();
        match Config::load(&dir.join(Config::FILE_NAME)) {
            // El primer arranque se decide en la bienvenida, no al leer el archivo.
            Ok(config) if config.theme.is_none() && self.config.theme.is_some() => {
                self.config = Config {
                    theme: self.config.theme,
                    ..config
                };
            }
            Ok(config) => self.config = config,
            Err(error) => self.show_error(error.to_string()),
        }
        // Otros servidores: se paran y se lanzan los nuevos en la siguiente sincronización.
        if self.config.lsp != previous_lsp {
            self.stop_lsp();
        }
    }

    /// Vuelve a leer los temas del usuario. El tema activo se reconstruye aunque conserve
    /// el nombre, porque su archivo puede haber cambiado.
    fn reload_themes(&mut self) {
        let Some(dir) = &self.data_dir else { return };
        let (themes, errors) = ThemeSet::load(&dir.join(THEMES_DIR));
        self.themes = themes;
        let appearance = self.appearance();
        self.active_theme =
            ActiveTheme::new(&self.themes, self.config.theme_name(appearance), appearance);
        if let Some(error) = errors.first() {
            self.show_error(error.to_string());
        }
    }

    fn save_config(&mut self) {
        let Some(dir) = &self.data_dir else { return };
        if let Err(error) = self.config.save(&dir.join(Config::FILE_NAME)) {
            self.show_error(error.to_string());
        }
    }

    fn show_error(&mut self, text: String) {
        self.notice = Some(Notice {
            text,
            is_error: true,
        });
    }

    fn show_info(&mut self, text: String) {
        self.notice = Some(Notice {
            text,
            is_error: false,
        });
    }

    // --- Edición ----------------------------------------------------------------------

    fn apply_editor_action(&mut self, id: DocumentId, action: EditorAction) {
        // La tecla Tab inserta lo que diga la configuración (espacios o tabulador).
        let action = match action {
            EditorAction::Indent => EditorAction::Insert(self.config.indent_unit()),
            other => other,
        };
        if matches!(
            action,
            EditorAction::Click { .. } | EditorAction::GoToDefinition { .. }
        ) {
            self.focus = Focus::Editor;
            self.context_menu = None;
            self.workspace.clear_selection();
            self.workspace.activate(id);
        }
        let Some(document) = self.workspace.document_mut(id) else {
            return;
        };
        let view = self.views.entry(id).or_default();
        // Si la vista cambia de tamaño (p. ej. al abrir o cerrar una barra) y el cursor
        // estaba visible, se mantiene visible con las nuevas medidas.
        let mut keep_cursor_visible = false;
        if let EditorAction::Viewport(viewport) = action {
            keep_cursor_visible = view.is_cursor_visible(&document.editor, view.viewport);
            view.viewport = viewport;
        }
        let result = apply_action(action, &mut document.editor, view, view.viewport);
        if keep_cursor_visible {
            view.reveal_cursor(&document.editor, view.viewport);
        }
        document.refresh_language();
        if let Err(error) = result {
            self.show_error(error.to_string());
        }
    }

    /// Lanza el resaltado de los documentos visibles que estén desactualizados y sin otro
    /// cálculo en marcha. Se ejecuta fuera del hilo de la interfaz; mientras tanto se pinta
    /// el resaltado anterior.
    fn schedule_highlight(&mut self) -> Task<Message> {
        let tasks: Vec<_> = self
            .visible_documents()
            .into_iter()
            .filter_map(|id| self.schedule_highlight_for(id))
            .collect();
        Task::batch(tasks)
    }

    fn schedule_highlight_for(&mut self, id: DocumentId) -> Option<Task<Message>> {
        let document = self.workspace.document(id)?;
        let revision = document.editor.version();
        let language = document.language();
        let state = self.highlights.entry(id).or_default();
        if state.pending || state.done == Some((revision, language)) {
            return None;
        }
        state.pending = true;
        let text = document.editor.text();
        Some(Task::perform(
            async move {
                lorca_syntax::highlight(language, &text)
                    .map(|highlights| highlights.map(Arc::new))
                    .map_err(|error: SyntaxError| error.to_string())
            },
            move |result| Message::Highlighted {
                id,
                revision,
                language,
                result,
            },
        ))
    }

    fn undo_redo(&mut self, undo: bool) {
        let Some(id) = self.workspace.active_id() else {
            return;
        };
        let Some(document) = self.workspace.document_mut(id) else {
            return;
        };
        let result = if undo {
            document.editor.undo()
        } else {
            document.editor.redo()
        };
        document.refresh_language();
        match result {
            Ok(_) => {
                let view = self.views.entry(id).or_default();
                view.reveal_cursor(&document.editor, view.viewport);
            }
            Err(error) => {
                self.notice = Some(Notice {
                    text: error.to_string(),
                    is_error: true,
                })
            }
        }
    }

    // --- Archivos ---------------------------------------------------------------------

    fn open_paths(&mut self, paths: &[PathBuf]) {
        let errors: Vec<String> = paths
            .iter()
            .filter_map(|path| self.workspace.open_file(path).err())
            .map(|error| error.to_string())
            .collect();
        if !errors.is_empty() {
            self.notice = Some(Notice {
                text: errors.join(" · "),
                is_error: true,
            });
        }
    }

    /// Guarda en la ruta actual o, si no la tiene, pide una.
    fn save(&mut self, id: DocumentId, then: Then) -> Task<Message> {
        let Some(document) = self.workspace.document(id) else {
            return Task::none();
        };
        if document.path().is_none() {
            return self.save_as(id, then);
        }
        match self.workspace.save(id) {
            Ok(()) => {
                self.notify_saved(id);
                self.finish(id, then)
            }
            Err(error) => self.abort_with_error(error.to_string()),
        }
    }

    fn save_as(&mut self, id: DocumentId, then: Then) -> Task<Message> {
        if self.dialog_open {
            return Task::none();
        }
        let Some(document) = self.workspace.document(id) else {
            return Task::none();
        };
        let file_name = match document.path() {
            Some(_) => document.display_name(),
            None => format!("{}.txt", document.display_name()),
        };
        let directory = document.path().and_then(|p| p.parent()).map(PathBuf::from);
        self.workspace.activate(id);
        self.dialog_open = true;
        Task::perform(dialogs::pick_save_path(file_name, directory), move |path| {
            Message::SavePathPicked { id, path, then }
        })
    }

    fn notify_saved(&mut self, id: DocumentId) {
        self.lsp_saved(id);
        if let Some(document) = self.workspace.document(id) {
            let text = format!("Guardado «{}»", document.display_name());
            self.notice = Some(Notice {
                text,
                is_error: false,
            });
        }
    }

    /// Muestra un error y cancela cualquier salida en curso.
    fn abort_with_error(&mut self, text: String) -> Task<Message> {
        self.notice = Some(Notice {
            text,
            is_error: true,
        });
        self.quit_queue = None;
        self.close_queue.clear();
        Task::none()
    }

    // --- Cierre -----------------------------------------------------------------------

    /// Cierra o sigue con la salida; si hay cambios sin guardar, pregunta antes.
    fn request_close(&mut self, id: DocumentId, then: Then) -> Task<Message> {
        if self.dialog_open {
            return Task::none();
        }
        let Some(document) = self.workspace.document(id) else {
            return self.finish(id, then);
        };
        if !document.is_modified() {
            return self.finish(id, then);
        }
        let name = document.display_name();
        self.workspace.activate(id);
        self.dialog_open = true;
        Task::perform(dialogs::confirm_save(name), move |choice| {
            Message::SaveChoiceMade { id, choice, then }
        })
    }

    fn finish(&mut self, id: DocumentId, then: Then) -> Task<Message> {
        match then {
            Then::Nothing => Task::none(),
            Then::CloseTab => {
                self.close_now(id);
                Task::none()
            }
            Then::CloseAndContinue => {
                self.close_now(id);
                self.process_close_queue()
            }
            Then::ContinueQuit => self.process_quit(),
        }
    }

    /// Pregunta por el siguiente documento modificado o sale si no queda ninguno.
    fn process_quit(&mut self) -> Task<Message> {
        let Some(queue) = self.quit_queue.as_mut() else {
            return Task::none();
        };
        match queue.pop_front() {
            Some(id) => self.request_close(id, Then::ContinueQuit),
            None => iced::exit(),
        }
    }

    /// Cierra la pestaña sin preguntar y limpia su estado de vista.
    fn close_now(&mut self, id: DocumentId) {
        self.workspace.close(id);
        self.views.remove(&id);
        self.highlights.remove(&id);
        self.marks.remove(&id);
        self.mark_cache.remove(&id);
        self.previews.remove(&id);
        self.preview_cache.remove(&id);
        self.ensure_document();
    }

    /// Reacciona a un cambio externo de un archivo abierto.
    fn file_changed(&mut self, path: &std::path::Path) -> Task<Message> {
        if let Some(dir) = &self.data_dir {
            if path == dir.join(Config::FILE_NAME) {
                self.reload_config();
            } else if path.parent() == Some(dir.join(THEMES_DIR).as_path()) {
                self.reload_themes();
            }
        }
        let Some(id) = self.workspace.find_by_path(path) else {
            return Task::none();
        };
        let Some(document) = self.workspace.document_mut(id) else {
            return Task::none();
        };
        let now = DiskStamp::of(path);
        // Nuestro propio guardado también genera eventos: la huella ya coincide.
        if now == document.disk_stamp() {
            return Task::none();
        }
        let name = document.display_name();
        match now {
            None => {
                document.mark_modified();
                document.acknowledge_disk_change();
                self.show_error(format!(
                    "«{name}» se ha borrado o movido en disco; guárdalo para conservarlo"
                ));
                Task::none()
            }
            Some(_) if !document.is_modified() => {
                self.reload_document(id);
                self.show_info(format!("«{name}» se ha recargado: cambió en disco"));
                Task::none()
            }
            Some(_) => {
                if self.dialog_open {
                    return Task::none();
                }
                self.dialog_open = true;
                Task::perform(dialogs::confirm_reload(name), move |reload| {
                    Message::ExternalChangeChoice { id, reload }
                })
            }
        }
    }

    /// Relee un documento de disco conservando, en lo posible, cursor y desplazamiento.
    fn reload_document(&mut self, id: DocumentId) {
        let selection = self.workspace.document(id).map(|d| d.editor.selection());
        if let Err(error) = self.workspace.reload(id) {
            self.show_error(error.to_string());
            return;
        }
        if let (Some(selection), Some(document)) = (selection, self.workspace.document_mut(id)) {
            document.editor.set_selection(selection);
        }
    }

    /// Mantiene siempre al menos una pestaña abierta.
    fn ensure_document(&mut self) {
        if self.workspace.is_empty() {
            self.workspace.new_document();
        }
    }

    // --- Vista ------------------------------------------------------------------------

    pub fn view(&self) -> Element<'_, Message> {
        if let Some(choice) = self.welcome_choice {
            return welcome::view(choice, self.system_mode, &self.themes, &self.config);
        }
        let mut layout = column![self.tab_bar()];
        if let Some(input) = &self.goto {
            let lines = self
                .workspace
                .active()
                .map_or(1, |d| d.editor.buffer().len_lines());
            layout = layout.push(find::goto_bar(input, lines));
        } else if self.find.open {
            let bookmarks = self
                .workspace
                .active()
                .map_or(0, |d| d.editor.bookmarks().len());
            layout = layout.push(find::bar(&self.find, self.find_counter(), bookmarks));
        }
        layout = layout.push(self.editor_area());
        if let Some(results) = &self.results {
            layout = layout.push(find::results_panel(results));
        }
        let base: Element<'_, Message> = layout.push(self.status_bar()).into();
        let base = self.lsp_popups(base);
        let base = match &self.context_menu {
            Some(menu) => {
                context_menu::layer(base, menu, self.menu_items(menu.target), self.window_size)
            }
            None => base,
        };
        match &self.palette {
            Some(state) => palette::layer(base, state, &self.palette_matches()),
            None => base,
        }
    }

    /// Desplegables de autocompletado e información junto al cursor del documento activo.
    fn lsp_popups<'a>(&'a self, base: Element<'a, Message>) -> Element<'a, Message> {
        let Some((id, point)) = self.lsp.cursor_point else {
            return base;
        };
        if self.workspace.active_id() != Some(id) || self.focus != Focus::Editor {
            return base;
        }
        let line_height = self.metrics().line_height;
        if let Some(completion) = self.lsp.completion.as_ref().filter(|c| c.document == id) {
            return crate::lsp_view::completion_layer(
                base,
                completion.rows().collect(),
                completion.selected,
                point,
                self.window_size,
                line_height,
            );
        }
        if let Some(hover) = self.lsp.hover.as_ref().filter(|h| h.document == id) {
            return crate::lsp_view::hover_layer(
                base,
                &hover.content,
                &self.active_theme.iced,
                point,
                self.window_size,
                line_height,
            );
        }
        base
    }

    fn tab_bar(&self) -> Element<'_, Message> {
        let tabs: Vec<Element<'_, Message>> =
            self.workspace.documents().map(|d| self.tab(d)).collect();
        let bar = scrollable(row(tabs).spacing(1)).direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(3).scroller_width(3),
        ));
        container(bar)
            .width(Length::Fill)
            .style(|theme: &Theme| {
                container::Style::default()
                    .background(theme.extended_palette().background.weakest.color)
            })
            .into()
    }

    fn tab<'a>(&self, document: &'a Document) -> Element<'a, Message> {
        let id = document.id();
        let is_active = self.workspace.active_id() == Some(id);
        let is_selected = self.workspace.is_selected(id);

        let mut content: Vec<Element<'a, Message>> =
            vec![text(document.display_name()).size(13).into()];
        if document.is_modified() {
            content.push(text("●").size(10).into());
        }
        content.push(
            button(text("×").size(14))
                .padding([0, 4])
                .style(button::text)
                .on_press(Message::CloseTab(id))
                .into(),
        );

        let is_hovered = self.hovered_tab == Some(id);
        let is_dragged = self.dragging_tab == Some(id);
        // Contenedor (no botón) para recibir la pulsación al momento y poder arrastrar.
        let tab = container(row(content).spacing(6).align_y(Alignment::Center))
            .padding([5, 10])
            .style(move |theme: &Theme| {
                tab_style(theme, is_active, is_selected, is_hovered || is_dragged)
            });
        let tab = mouse_area(tab)
            .on_press(Message::TabPressed(id))
            .on_middle_press(Message::CloseTab(id))
            .on_enter(Message::TabHovered(id))
            .on_exit(Message::TabUnhovered(id))
            .interaction(iced::mouse::Interaction::Pointer);
        let tab = context_area(tab, move |position| Message::TabContextMenu(id, position));

        match document.path() {
            Some(path) => tooltip(
                tab,
                container(text(path.display().to_string()).size(12))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .into(),
            None => tab,
        }
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let mut items: Vec<Element<'_, Message>> = Vec::new();

        if let Some(notice) = &self.notice {
            let is_error = notice.is_error;
            items.push(
                button(text(notice.text.clone()).size(12))
                    .padding(0)
                    .style(move |theme: &Theme, status| {
                        let mut style = button::text(theme, status);
                        if is_error {
                            style.text_color = theme.extended_palette().danger.base.color;
                        }
                        style
                    })
                    .on_press(Message::DismissNotice)
                    .into(),
            );
        }
        if self.notice.is_none()
            && let Some((severity, message)) = self.diagnostic_at_cursor()
        {
            let is_error = severity == lorca_lsp::Severity::Error;
            let first_line = message.lines().next().unwrap_or_default().to_owned();
            items.push(
                text(first_line)
                    .size(12)
                    .wrapping(text::Wrapping::None)
                    .style(move |theme: &Theme| {
                        let palette = theme.extended_palette();
                        text::Style {
                            color: Some(if is_error {
                                palette.danger.base.color
                            } else {
                                palette.warning.base.color
                            }),
                        }
                    })
                    .into(),
            );
        }
        items.push(space::horizontal().into());

        if let Some((name, ready)) = self.lsp_status() {
            let label = if ready {
                name
            } else {
                format!("{name} (iniciando…)")
            };
            items.push(text(label).size(12).into());
        }
        if let Some((errors, warnings)) = self.diagnostic_counts() {
            items.push(
                button(text(format!("✕ {errors}  ⚠ {warnings}")).size(12))
                    .padding(0)
                    .style(button::text)
                    .on_press(Message::NextProblem(true))
                    .into(),
            );
        }

        if let Some(document) = self.workspace.active() {
            let cursor = document.editor.cursor();
            let selected = document.editor.selection().range().len();
            let mut position = format!("Lín {}, Col {}", cursor.line + 1, cursor.column + 1);
            let cursors = document.editor.selections().len();
            if cursors > 1 {
                position.push_str(&format!(" · {cursors} cursores"));
            } else if selected > 0 {
                position.push_str(&format!(" ({selected} seleccionados)"));
            }
            let format = document.format();
            items.push(text(position).size(12).into());
            items.push(
                pick_list(
                    Language::ALL,
                    Some(document.language()),
                    Message::LanguageSelected,
                )
                .text_size(12)
                .padding([0, 6])
                .into(),
            );
            let id = document.id();
            items.push(click_area(
                text(format.encoding.label()).size(12),
                move |position| Message::StatusMenu(MenuTarget::Encoding(id), position),
            ));
            items.push(click_area(
                text(format.line_ending.label()).size(12),
                move |position| Message::StatusMenu(MenuTarget::LineEnding(id), position),
            ));
        }
        items.push(click_area(text("¶").size(13), |position| {
            Message::StatusMenu(MenuTarget::View, position)
        }));
        let theme_label = format!(
            "{} · {}",
            self.active_theme.name(),
            self.config.theme.unwrap_or_default().label()
        );
        items.push(click_area(text(theme_label).size(12), |position| {
            Message::StatusMenu(MenuTarget::Theme, position)
        }));

        container(row(items).spacing(18).align_y(Alignment::Center))
            .padding([3, 12])
            .width(Length::Fill)
            .style(|theme: &Theme| {
                container::Style::default()
                    .background(theme.extended_palette().background.weakest.color)
            })
            .into()
    }
}

fn tab_style(
    theme: &Theme,
    is_active: bool,
    is_selected: bool,
    is_hovered: bool,
) -> container::Style {
    let palette = theme.extended_palette();
    let (background, text_color) = if is_selected {
        (palette.primary.weak.color, palette.primary.weak.text)
    } else if is_active {
        (palette.background.base.color, palette.background.base.text)
    } else if is_hovered {
        (
            palette.background.weak.color,
            palette.background.strong.text,
        )
    } else {
        (
            palette.background.weakest.color,
            palette.background.strong.text,
        )
    };
    container::Style::default()
        .background(background)
        .color(text_color)
}

/// Flechas arriba/abajo mientras la paleta está abierta (el campo de texto no las usa).
fn palette_keys(
    event: iced::Event,
    _status: event::Status,
    _window: window::Id,
) -> Option<Message> {
    use iced::keyboard::key::Named;
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::ArrowUp),
            ..
        }) => Some(Message::PaletteMove(-1)),
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::ArrowDown),
            ..
        }) => Some(Message::PaletteMove(1)),
        _ => None,
    }
}

/// `true` si el mensaje puede cambiar algo que se guarda en la sesión.
fn affects_session(message: &Message) -> bool {
    !matches!(
        message,
        Message::Tick
            | Message::SystemTheme(_)
            | Message::DismissNotice
            | Message::CloseRequested
            | Message::ThemeSelected(_)
            | Message::WelcomeChoose(_)
            | Message::WelcomeDone
            | Message::Highlighted { .. }
            | Message::PdfExported(_)
            | Message::InstallMenu
            | Message::ModifiersChanged(_)
            | Message::TabUnhovered(_)
            | Message::MouseReleased
            | Message::OpenPalette
            | Message::PaletteInput(_)
            | Message::PaletteMove(_)
            | Message::ClosePalette
            | Message::WindowResized(_)
            | Message::CloseContextMenu
            | Message::LspConnected(_)
            | Message::Lsp(_)
            | Message::ShowHover
            | Message::Editor(_, EditorAction::CursorPoint(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn housekeeping_messages_do_not_dirty_session() {
        assert!(!affects_session(&Message::Tick));
        assert!(!affects_session(&Message::SystemTheme(Mode::Dark)));
        assert!(affects_session(&Message::NewDocument));
        let mut workspace = Workspace::new();
        let id = workspace.new_document();
        assert!(affects_session(&Message::Editor(
            id,
            EditorAction::SelectAll
        )));
    }
}
