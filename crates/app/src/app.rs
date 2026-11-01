//! Estado de la aplicación, mensajes y vista principal.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

use iced::widget::{button, column, container, mouse_area, row, scrollable, space, text, tooltip};
use iced::{Alignment, Element, Length, Subscription, Task, Theme, keyboard, window};
use lorca_workspace::{Document, DocumentId, Workspace};

use crate::dialogs::{self, SaveChoice};
use crate::editor::{DocView, EditorAction, Viewport, apply_action, code_editor};
use crate::shortcuts::shortcut;

/// Qué hacer con un documento cuando termina el flujo de guardado o confirmación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    /// Solo guardar.
    Nothing,
    /// Cerrar la pestaña.
    CloseTab,
    /// Seguir con el siguiente documento antes de salir.
    ContinueQuit,
}

#[derive(Debug, Clone)]
pub enum Message {
    Editor(EditorAction),
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
    ActivateTab(DocumentId),
    NextTab,
    PreviousTab,
    Undo,
    Redo,
    CloseRequested,
    DismissNotice,
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
    viewport: Viewport,
    notice: Option<Notice>,
    /// Hay un diálogo nativo abierto: se ignoran las órdenes que abrirían otro.
    dialog_open: bool,
    /// Documentos con cambios que quedan por confirmar antes de salir.
    quit_queue: Option<VecDeque<DocumentId>>,
}

impl Lorca {
    /// Estado inicial: abre los archivos pasados por línea de órdenes o un documento nuevo.
    pub fn boot() -> (Self, Task<Message>) {
        let mut app = Self {
            workspace: Workspace::new(),
            views: HashMap::new(),
            viewport: Viewport::default(),
            notice: None,
            dialog_open: false,
            quit_queue: None,
        };
        let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
        app.open_paths(&paths);
        app.ensure_document();
        (app, Task::none())
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
        Theme::Dark
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            keyboard::listen().filter_map(shortcut),
            window::close_requests().map(|_| Message::CloseRequested),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Editor(action) => {
                self.apply_editor_action(action);
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
                        Task::none()
                    }
                }
            }
            Message::ActivateTab(id) => {
                self.workspace.activate(id);
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
                self.quit_queue = Some(self.workspace.modified_documents().into());
                self.process_quit()
            }
            Message::DismissNotice => {
                self.notice = None;
                Task::none()
            }
        }
    }

    // --- Edición ----------------------------------------------------------------------

    fn apply_editor_action(&mut self, action: EditorAction) {
        if let EditorAction::Viewport(viewport) = action {
            self.viewport = viewport;
        }
        let Some(id) = self.workspace.active_id() else {
            return;
        };
        let Some(document) = self.workspace.document_mut(id) else {
            return;
        };
        let view = self.views.entry(id).or_default();
        if let Err(error) = apply_action(action, &mut document.editor, view, self.viewport) {
            self.notice = Some(Notice {
                text: error.to_string(),
                is_error: true,
            });
        }
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
        match result {
            Ok(_) => self
                .views
                .entry(id)
                .or_default()
                .reveal_cursor(&document.editor, self.viewport),
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
                self.workspace.close(id);
                self.views.remove(&id);
                self.ensure_document();
                Task::none()
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

    /// Mantiene siempre al menos una pestaña abierta.
    fn ensure_document(&mut self) {
        if self.workspace.is_empty() {
            self.workspace.new_document();
        }
    }

    // --- Vista ------------------------------------------------------------------------

    pub fn view(&self) -> Element<'_, Message> {
        column![self.tab_bar(), self.editor_area(), self.status_bar()].into()
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
            .on_press(Message::ActivateTab(id))
            .style(move |theme: &Theme, status| tab_style(theme, status, is_active));
        let tab = mouse_area(tab).on_middle_press(Message::CloseTab(id));

        match document.path() {
            Some(path) => tooltip(
                tab,
                container(text(path.display().to_string()).size(12))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .into(),
            None => tab.into(),
        }
    }

    fn editor_area(&self) -> Element<'_, Message> {
        let Some(document) = self.workspace.active() else {
            return space().into();
        };
        let view = self.views.get(&document.id()).copied().unwrap_or_default();
        code_editor(&document.editor, view, !self.dialog_open, Message::Editor).into()
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
            for label in [
                position,
                document.language().name().to_owned(),
                format.encoding.label().to_owned(),
                format.line_ending.label().to_owned(),
            ] {
                items.push(text(label).size(12).into());
            }
        }

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

fn tab_style(theme: &Theme, status: button::Status, is_active: bool) -> button::Style {
    let palette = theme.extended_palette();
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
