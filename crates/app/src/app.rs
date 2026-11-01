//! Estado de la aplicación, mensajes y vista principal.

use std::collections::{HashMap, VecDeque};
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
use lorca_search::SearchQuery;
use lorca_syntax::{Highlights, Language, SyntaxError};
use lorca_workspace::{Config, Document, DocumentId, SessionStore, ThemeMode, Workspace, data_dir};

use crate::context_menu::{self, ContextAction, ContextMenu, MenuTarget, context_area};
use crate::dialogs::{self, SaveChoice};
use crate::editor::{DocView, EditorAction, apply_action};
use crate::find::{self, FindMessage, FindState, SearchResults};
use crate::shortcuts::shortcut;
use crate::welcome;

#[path = "find_actions.rs"]
mod find_actions;

pub use find_actions::BookmarkAction;
use find_actions::{Focus, MatchCache};

#[path = "panes.rs"]
mod panes;

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
    Context(ContextAction),
    CloseContextMenu,
    Paste(Option<String>),
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
    window_size: Size,
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
            window_size: WINDOW_SIZE,
        };
        if !problems.is_empty() {
            app.show_error(problems.join(" · "));
        }
        let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
        app.open_paths(&paths);
        app.ensure_document();
        app.sync_panes();
        let highlight = app.schedule_highlight();
        (
            app,
            Task::batch([system::theme().map(Message::SystemTheme), highlight]),
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
        let mode = self
            .welcome_choice
            .or(self.config.theme)
            .unwrap_or_default();
        resolve_theme(mode, self.system_mode)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            event::listen_with(shortcut),
            window::close_requests().map(|_| Message::CloseRequested),
            system::theme_changes().map(Message::SystemTheme),
            window::resize_events().map(|(_, size)| Message::WindowResized(size)),
        ];
        if self.session_dirty.is_some() {
            subscriptions.push(iced::time::every(SESSION_TICK).map(|_| Message::Tick));
        }
        Subscription::batch(subscriptions)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        let task = self.handle(message);
        self.sync_panes();
        self.refresh_search();
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
                if let EditorAction::ContextMenu { position, .. } = action {
                    self.open_context_menu(position, MenuTarget::Text(id));
                }
                self.apply_editor_action(id, action);
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
                if self.context_menu.take().is_none() {
                    self.escape();
                }
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
        if matches!(action, EditorAction::Click { .. }) {
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
        let revision = document.editor.revision();
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
        self.ensure_document();
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
            return welcome::view(choice, self.system_mode);
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
        match &self.context_menu {
            Some(menu) => {
                context_menu::layer(base, menu, self.menu_items(menu.target), self.window_size)
            }
            None => base,
        }
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

        let tab = button(row(content).spacing(6).align_y(Alignment::Center))
            .padding([5, 10])
            .on_press(Message::TabPressed(id))
            .style(move |theme: &Theme, status| tab_style(theme, status, is_active, is_selected));
        let tab = context_area(
            mouse_area(tab).on_middle_press(Message::CloseTab(id)),
            move |position| Message::TabContextMenu(id, position),
        );

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
        items.push(space::horizontal().into());

        if let Some(document) = self.workspace.active() {
            let cursor = document.editor.cursor();
            let selected = document.editor.selection().range().len();
            let mut position = format!("Lín {}, Col {}", cursor.line + 1, cursor.column + 1);
            if selected > 0 {
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
            items.push(text(format.encoding.label()).size(12).into());
            items.push(text(format.line_ending.label()).size(12).into());
        }
        items.push(
            pick_list(
                ThemeMode::ALL,
                Some(self.config.theme.unwrap_or_default()),
                Message::ThemeSelected,
            )
            .text_size(12)
            .padding([0, 6])
            .into(),
        );

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
    status: button::Status,
    is_active: bool,
    is_selected: bool,
) -> button::Style {
    let palette = theme.extended_palette();
    if is_selected {
        return button::Style {
            background: Some(palette.primary.weak.color.into()),
            text_color: palette.primary.weak.text,
            ..button::Style::default()
        };
    }
    let background = if is_active {
        palette.background.base.color
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        palette.background.weak.color
    } else {
        palette.background.weakest.color
    };
    let text_color = if is_active {
        palette.background.base.text
    } else {
        palette.background.strong.text
    };
    button::Style {
        background: Some(background.into()),
        text_color,
        ..button::Style::default()
    }
}

/// Tema de iced para un modo; en automático sigue al sistema (claro si no lo indica).
pub fn resolve_theme(mode: ThemeMode, system: Mode) -> Theme {
    match (mode, system) {
        (ThemeMode::Dark, _) | (ThemeMode::Auto, Mode::Dark) => Theme::Dark,
        (ThemeMode::Light, _) | (ThemeMode::Auto, _) => Theme::Light,
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
            | Message::ModifiersChanged(_)
            | Message::WindowResized(_)
            | Message::CloseContextMenu
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_theme_follows_system() {
        assert_eq!(resolve_theme(ThemeMode::Auto, Mode::Dark), Theme::Dark);
        assert_eq!(resolve_theme(ThemeMode::Auto, Mode::Light), Theme::Light);
        assert_eq!(resolve_theme(ThemeMode::Auto, Mode::None), Theme::Light);
    }

    #[test]
    fn explicit_theme_ignores_system() {
        assert_eq!(resolve_theme(ThemeMode::Light, Mode::Dark), Theme::Light);
        assert_eq!(resolve_theme(ThemeMode::Dark, Mode::Light), Theme::Dark);
    }

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
