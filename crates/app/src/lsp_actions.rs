//! Servidores de lenguaje sobre `Lorca`: arranque, sincronización de documentos,
//! diagnósticos, autocompletado, información y definición.
//!
//! Es un submódulo de `app` para poder usar su estado privado.

use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::{Path, PathBuf};

use iced::Point;
use iced::futures::channel::mpsc::UnboundedSender;
use iced::widget::markdown;
use lorca_core::Buffer;
use lorca_lsp::{
    Client, CompletionItem, Diagnostic, Event, Position, PositionEncoding, ServerCommand, ServerId,
    Severity, find_root, known_servers, language_id, servers::find_executable,
};
use lorca_syntax::Language;
use lorca_workspace::DocumentId;

use super::Lorca;
use crate::editor::{CompletionKey, Underline, UnderlineKind};
use crate::palette::fuzzy_score;

/// Propuestas que se muestran a la vez en el desplegable.
pub const COMPLETION_ROWS: usize = 10;
/// Letras de una palabra a partir de las cuales se pide autocompletado al escribir.
const AUTO_COMPLETE_AFTER: usize = 2;

/// Un servidor en marcha.
struct ServerEntry {
    client: Client,
    command: ServerCommand,
    root: PathBuf,
}

/// Documento abierto en un servidor y última versión enviada.
struct SyncedDoc {
    server: ServerId,
    path: PathBuf,
    language: Language,
    version: u64,
}

/// Diagnósticos publicados para un archivo.
struct Published {
    server: ServerId,
    items: Vec<Diagnostic>,
    generation: u64,
}

/// Diagnósticos de un documento convertidos a caracteres para una versión del texto.
struct Converted {
    version: u64,
    generation: u64,
    underlines: Vec<Underline>,
    messages: Vec<(Range<usize>, Severity, String)>,
}

/// Desplegable de autocompletado abierto.
#[derive(Debug)]
pub struct Completion {
    pub document: DocumentId,
    server: ServerId,
    /// Índice donde empieza lo que se está completando.
    anchor: usize,
    items: Vec<CompletionItem>,
    /// Índices de `items` que encajan con lo escrito, de mejor a peor.
    visible: Vec<usize>,
    pub selected: usize,
}

impl Completion {
    /// Propuestas visibles con su posición en la lista.
    pub fn rows(&self) -> impl Iterator<Item = (usize, &CompletionItem)> {
        let first = self.selected.saturating_sub(COMPLETION_ROWS - 1);
        self.visible
            .iter()
            .enumerate()
            .skip(first)
            .take(COMPLETION_ROWS)
            .filter_map(|(row, &index)| Some((row, self.items.get(index)?)))
    }
}

/// Información (hover) que se está mostrando.
#[derive(Debug)]
pub struct HoverPopup {
    pub document: DocumentId,
    pub content: markdown::Content,
}

/// Una petición en espera de respuesta.
#[derive(Debug, Clone, Copy)]
struct Request {
    server: ServerId,
    id: u64,
    document: DocumentId,
    /// Para el autocompletado: inicio de la palabra.
    anchor: usize,
}

/// Estado de los servidores de lenguaje.
#[derive(Default)]
pub struct LspState {
    sender: Option<UnboundedSender<Event>>,
    servers: HashMap<ServerId, ServerEntry>,
    next_server: u64,
    /// Órdenes que no arrancaron o que se cerraron solas: no se reintentan.
    broken: HashSet<ServerCommand>,
    /// Servidor elegido para cada lenguaje (`None`: ninguno disponible).
    resolved: HashMap<Language, Option<ServerCommand>>,
    documents: HashMap<DocumentId, SyncedDoc>,
    diagnostics: HashMap<PathBuf, Published>,
    generation: u64,
    converted: HashMap<DocumentId, Converted>,
    pub completion: Option<Completion>,
    pub hover: Option<HoverPopup>,
    completion_request: Option<Request>,
    hover_request: Option<Request>,
    definition_request: Option<Request>,
    /// Punto de la ventana bajo el cursor del editor con foco.
    pub cursor_point: Option<(DocumentId, Point)>,
}

/// Columna del protocolo para un índice de carácter del buffer.
fn to_position(buffer: &Buffer, index: usize, encoding: PositionEncoding) -> Position {
    let position = buffer.char_to_position(index);
    let line = buffer.line_text(position.line);
    Position::new(
        u32::try_from(position.line).unwrap_or(u32::MAX),
        encoding.to_server(&line, position.column),
    )
}

/// Índice de carácter del buffer para una posición del protocolo.
fn to_index(buffer: &Buffer, position: Position, encoding: PositionEncoding) -> usize {
    let line = usize::try_from(position.line).unwrap_or(usize::MAX);
    let line = line.min(buffer.len_lines().saturating_sub(1));
    let column = encoding.to_chars(&buffer.line_text(line), position.character);
    buffer.position_to_char(lorca_core::Position::new(line, column))
}

fn is_identifier(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Inicio de la palabra (identificador) que acaba en `index`.
fn word_start(buffer: &Buffer, index: usize) -> usize {
    let position = buffer.char_to_position(index);
    let line: Vec<char> = buffer.line_text(position.line).chars().collect();
    let mut column = position.column.min(line.len());
    while column > 0 && line.get(column - 1).is_some_and(|c| is_identifier(*c)) {
        column -= 1;
    }
    index - (position.column - column)
}

/// Propuestas que encajan con `prefix`, de mejor a peor: puntuación difusa y, a igualdad,
/// el orden que sugiere el servidor.
pub fn filter_completions(items: &[CompletionItem], prefix: &str) -> Vec<usize> {
    let mut scored: Vec<(u32, &str, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            let score = fuzzy_score(prefix, &item.filter_text)?;
            Some((score, item.sort_text.as_str(), index))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored.into_iter().map(|(_, _, index)| index).collect()
}

fn underline_kind(severity: Severity) -> UnderlineKind {
    match severity {
        Severity::Error => UnderlineKind::Error,
        Severity::Warning => UnderlineKind::Warning,
        Severity::Information | Severity::Hint => UnderlineKind::Info,
    }
}

impl Lorca {
    fn lsp_version(version: u64) -> i32 {
        i32::try_from(version % (i32::MAX as u64)).unwrap_or(0)
    }

    /// Servidor que corresponde a un lenguaje según la configuración y lo instalado.
    fn lsp_command(&mut self, language: Language) -> Option<ServerCommand> {
        if let Some(cached) = self.lsp.resolved.get(&language) {
            return cached.clone();
        }
        let chosen = match self.config.lsp.servers.get(language.id()) {
            Some(server) if !server.enabled || server.command.trim().is_empty() => None,
            Some(server) => Some(ServerCommand {
                command: server.command.clone(),
                args: server.args.clone(),
            }),
            None => known_servers(language)
                .into_iter()
                .find(|c| find_executable(&c.command).is_some()),
        };
        self.lsp.resolved.insert(language, chosen.clone());
        chosen
    }

    /// Servidor en marcha para `path` con `command`, o uno nuevo.
    fn lsp_server_for(&mut self, command: &ServerCommand, path: &Path) -> Option<ServerId> {
        if let Some((id, _)) = self
            .lsp
            .servers
            .iter()
            .find(|(_, entry)| entry.command == *command && path.starts_with(&entry.root))
        {
            return Some(*id);
        }
        if self.lsp.broken.contains(command) {
            return None;
        }
        let sender = self.lsp.sender.clone()?;
        let root = find_root(path);
        self.lsp.next_server += 1;
        let id = ServerId(self.lsp.next_server);
        let started = Client::spawn(id, command, &root, move |event| {
            let _ = sender.unbounded_send(event);
        });
        match started {
            Ok(client) => {
                self.lsp.servers.insert(
                    id,
                    ServerEntry {
                        client,
                        command: command.clone(),
                        root,
                    },
                );
                Some(id)
            }
            Err(error) => {
                self.lsp.broken.insert(command.clone());
                self.show_error(error.to_string());
                None
            }
        }
    }

    fn lsp_client(&self, server: ServerId) -> Option<&Client> {
        self.lsp.servers.get(&server).map(|entry| &entry.client)
    }

    /// Abre, actualiza o cierra en los servidores los documentos del espacio de trabajo.
    pub(super) fn sync_lsp(&mut self) {
        if !self.config.lsp.enabled {
            if !self.lsp.servers.is_empty() || !self.lsp.documents.is_empty() {
                self.stop_lsp();
            }
            return;
        }
        if self.lsp.sender.is_none() {
            return;
        }
        let ids: Vec<DocumentId> = self.workspace.documents().map(|d| d.id()).collect();
        let open: HashSet<DocumentId> = ids.iter().copied().collect();
        let closed: Vec<DocumentId> = self
            .lsp
            .documents
            .keys()
            .filter(|id| !open.contains(id))
            .copied()
            .collect();
        for id in closed {
            self.close_lsp_document(id);
        }
        for id in ids {
            self.sync_lsp_document(id);
        }
    }

    fn close_lsp_document(&mut self, id: DocumentId) {
        let Some(doc) = self.lsp.documents.remove(&id) else {
            return;
        };
        if let Some(client) = self.lsp_client(doc.server) {
            client.did_close(&doc.path);
        }
        self.lsp.diagnostics.remove(&doc.path);
        self.lsp.converted.remove(&id);
    }

    /// Envía al servidor el estado actual de un documento si cambió.
    fn sync_lsp_document(&mut self, id: DocumentId) {
        let Some(document) = self.workspace.document(id) else {
            return;
        };
        let (path, language, version) = match document.path() {
            Some(path) => (
                path.to_owned(),
                document.language(),
                document.editor.version(),
            ),
            None => {
                self.close_lsp_document(id);
                return;
            }
        };
        if let Some(synced) = self.lsp.documents.get(&id) {
            if synced.path == path && synced.language == language {
                if synced.version != version {
                    let text = document.editor.text();
                    if let Some(client) = self.lsp_client(synced.server) {
                        client.did_change(&path, Self::lsp_version(version), &text);
                    }
                    if let Some(synced) = self.lsp.documents.get_mut(&id) {
                        synced.version = version;
                    }
                }
                return;
            }
            // Cambió la ruta («Guardar como») o el lenguaje: se cierra y se vuelve a abrir.
            self.close_lsp_document(id);
        }
        let Some(command) = self.lsp_command(language) else {
            return;
        };
        let Some(server) = self.lsp_server_for(&command, &path) else {
            return;
        };
        let Some(document) = self.workspace.document(id) else {
            return;
        };
        let text = document.editor.text();
        if let Some(client) = self.lsp_client(server) {
            client.did_open(
                &path,
                language_id(language, Some(&path)),
                Self::lsp_version(version),
                &text,
            );
        }
        self.lsp.documents.insert(
            id,
            SyncedDoc {
                server,
                path,
                language,
                version,
            },
        );
    }

    /// Avisa al servidor de que se guardó un documento.
    pub(super) fn lsp_saved(&mut self, id: DocumentId) {
        self.sync_lsp_document(id);
        let (Some(doc), Some(document)) =
            (self.lsp.documents.get(&id), self.workspace.document(id))
        else {
            return;
        };
        if let Some(client) = self.lsp_client(doc.server) {
            client.did_save(&doc.path, &document.editor.text());
        }
    }

    /// Para todos los servidores y olvida su estado (se volverán a lanzar si hace falta).
    pub(super) fn stop_lsp(&mut self) {
        for (_, mut entry) in self.lsp.servers.drain() {
            entry.client.shutdown();
        }
        let sender = self.lsp.sender.take();
        self.lsp = LspState {
            sender,
            next_server: self.lsp.next_server,
            ..LspState::default()
        };
    }

    /// Nombre del servidor del documento activo y si ya está listo.
    pub(super) fn lsp_status(&self) -> Option<(String, bool)> {
        let id = self.workspace.active_id()?;
        let doc = self.lsp.documents.get(&id)?;
        let entry = self.lsp.servers.get(&doc.server)?;
        Some((entry.command.command.clone(), entry.client.is_ready()))
    }

    // --- Diagnósticos -----------------------------------------------------------------

    /// Diagnósticos de un documento en caracteres, recalculados si cambió el texto o
    /// llegaron diagnósticos nuevos.
    pub(super) fn refresh_diagnostics(&mut self) {
        let ids: Vec<DocumentId> = self.lsp.documents.keys().copied().collect();
        for id in ids {
            let Some(doc) = self.lsp.documents.get(&id) else {
                continue;
            };
            let Some(published) = self.lsp.diagnostics.get(&doc.path) else {
                self.lsp.converted.remove(&id);
                continue;
            };
            let Some(document) = self.workspace.document(id) else {
                continue;
            };
            let version = document.editor.version();
            if self
                .lsp
                .converted
                .get(&id)
                .is_some_and(|c| c.version == version && c.generation == published.generation)
            {
                continue;
            }
            let encoding = self
                .lsp_client(published.server)
                .map(Client::encoding)
                .unwrap_or_default();
            let buffer = document.editor.buffer();
            let mut messages: Vec<(Range<usize>, Severity, String)> = published
                .items
                .iter()
                .map(|d| {
                    let start = to_index(buffer, d.range.start, encoding);
                    let end = to_index(buffer, d.range.end, encoding).max(start);
                    let message = match &d.source {
                        Some(source) => format!("{} ({source})", d.message),
                        None => d.message.clone(),
                    };
                    (start..end, d.severity, message)
                })
                .collect();
            messages.sort_by_key(|(range, severity, _)| (range.start, *severity));
            let underlines = messages
                .iter()
                .filter(|(_, severity, _)| *severity != Severity::Hint)
                .map(|(range, severity, _)| Underline {
                    range: range.clone(),
                    kind: underline_kind(*severity),
                })
                .collect();
            self.lsp.converted.insert(
                id,
                Converted {
                    version,
                    generation: published.generation,
                    underlines,
                    messages,
                },
            );
        }
    }

    pub(super) fn underlines(&self, id: DocumentId) -> &[Underline] {
        self.lsp
            .converted
            .get(&id)
            .map_or(&[][..], |c| c.underlines.as_slice())
    }

    /// Errores y avisos del documento activo.
    pub(super) fn diagnostic_counts(&self) -> Option<(usize, usize)> {
        let id = self.workspace.active_id()?;
        let converted = self.lsp.converted.get(&id)?;
        let count = |wanted: Severity| {
            converted
                .messages
                .iter()
                .filter(|(_, s, _)| *s == wanted)
                .count()
        };
        Some((count(Severity::Error), count(Severity::Warning)))
    }

    /// Mensaje del diagnóstico más grave bajo el cursor del documento activo.
    pub(super) fn diagnostic_at_cursor(&self) -> Option<(Severity, &str)> {
        let document = self.workspace.active()?;
        let converted = self.lsp.converted.get(&document.id())?;
        let head = document.editor.selection().head;
        converted
            .messages
            .iter()
            .filter(|(range, _, _)| range.start <= head && head <= range.end)
            .min_by_key(|(_, severity, _)| *severity)
            .map(|(_, severity, message)| (*severity, message.as_str()))
    }

    /// Lleva el cursor al siguiente (o anterior) diagnóstico y muestra su mensaje.
    pub(super) fn jump_to_diagnostic(&mut self, forward: bool) {
        let Some(id) = self.workspace.active_id() else {
            return;
        };
        let starts: Vec<usize> = match self.lsp.converted.get(&id) {
            Some(converted) if !converted.messages.is_empty() => converted
                .messages
                .iter()
                .map(|(range, _, _)| range.start)
                .collect(),
            _ => {
                self.show_info("No hay problemas en este documento".to_owned());
                return;
            }
        };
        let Some(document) = self.workspace.document_mut(id) else {
            return;
        };
        let head = document.editor.selection().head;
        let target = if forward {
            starts
                .iter()
                .find(|&&s| s > head)
                .or_else(|| starts.first())
        } else {
            starts
                .iter()
                .rev()
                .find(|&&s| s < head)
                .or_else(|| starts.last())
        };
        if let Some(&target) = target {
            document.editor.set_cursor(target, false);
        }
        self.reveal_active_cursor();
        if let Some((severity, message)) = self.diagnostic_at_cursor() {
            let message = message.to_owned();
            if severity == Severity::Error {
                self.show_error(message);
            } else {
                self.show_info(message);
            }
        }
    }

    // --- Eventos del servidor ---------------------------------------------------------

    pub(super) fn handle_lsp_event(&mut self, event: Event) {
        match event {
            Event::Ready { .. } => {}
            Event::Diagnostics {
                server,
                path,
                diagnostics,
            } => {
                self.lsp.generation += 1;
                self.lsp.diagnostics.insert(
                    path,
                    Published {
                        server,
                        items: diagnostics,
                        generation: self.lsp.generation,
                    },
                );
            }
            Event::Completion {
                server,
                request,
                items,
            } => self.completion_arrived(server, request, items),
            Event::Hover {
                server,
                request,
                text,
            } => {
                let Some(pending) = self.lsp.hover_request.take() else {
                    return;
                };
                if pending.server != server || pending.id != request {
                    self.lsp.hover_request = Some(pending);
                    return;
                }
                match text {
                    Some(text) => {
                        self.lsp.hover = Some(HoverPopup {
                            document: pending.document,
                            content: markdown::Content::parse(&text),
                        });
                    }
                    None => self.show_info("Sin información para esta posición".to_owned()),
                }
            }
            Event::Definition {
                server,
                request,
                locations,
            } => {
                let Some(pending) = self.lsp.definition_request.take() else {
                    return;
                };
                if pending.server != server || pending.id != request {
                    self.lsp.definition_request = Some(pending);
                    return;
                }
                let Some(location) = locations.into_iter().next() else {
                    self.show_info("No se encontró la definición".to_owned());
                    return;
                };
                let encoding = self
                    .lsp_client(server)
                    .map(Client::encoding)
                    .unwrap_or_default();
                match self.workspace.open_file(&location.path) {
                    Ok(id) => {
                        if let Some(document) = self.workspace.document_mut(id) {
                            let index =
                                to_index(document.editor.buffer(), location.range.start, encoding);
                            document.editor.set_cursor(index, false);
                        }
                        self.focus = super::Focus::Editor;
                        self.reveal_active_cursor();
                    }
                    Err(error) => self.show_error(error.to_string()),
                }
            }
            Event::Message {
                server,
                text,
                is_error,
            } => {
                let name = self
                    .lsp
                    .servers
                    .get(&server)
                    .map_or_else(String::new, |e| format!("{}: ", e.command.command));
                if is_error {
                    self.show_error(format!("{name}{text}"));
                } else {
                    self.show_info(format!("{name}{text}"));
                }
            }
            Event::Failed {
                server,
                request,
                message,
            } => {
                let is_ours = |pending: Option<Request>| {
                    pending.is_some_and(|p| p.server == server && Some(p.id) == request)
                };
                // Los fallos del autocompletado no se muestran: se escribe muy deprisa.
                if is_ours(self.lsp.completion_request) {
                    self.lsp.completion_request = None;
                } else if is_ours(self.lsp.hover_request) || is_ours(self.lsp.definition_request) {
                    self.lsp.hover_request = None;
                    self.lsp.definition_request = None;
                    self.show_error(message);
                } else if request.is_none() {
                    self.show_error(message);
                }
            }
            Event::Exited { server, message } => {
                let Some(entry) = self.lsp.servers.remove(&server) else {
                    return;
                };
                self.lsp.broken.insert(entry.command.clone());
                let pending_here =
                    |request: Option<Request>| request.is_some_and(|r| r.server == server);
                if pending_here(self.lsp.completion_request) {
                    self.lsp.completion_request = None;
                }
                if pending_here(self.lsp.hover_request) {
                    self.lsp.hover_request = None;
                }
                if pending_here(self.lsp.definition_request) {
                    self.lsp.definition_request = None;
                }
                self.show_error(format!(
                    "El servidor «{}» se cerró: {message}. Usa «LSP: Reiniciar servidores» para volver a lanzarlo",
                    entry.command.command
                ));
                let orphans: Vec<DocumentId> = self
                    .lsp
                    .documents
                    .iter()
                    .filter(|(_, d)| d.server == server)
                    .map(|(id, _)| *id)
                    .collect();
                for id in orphans {
                    if let Some(doc) = self.lsp.documents.remove(&id) {
                        self.lsp.diagnostics.remove(&doc.path);
                    }
                    self.lsp.converted.remove(&id);
                }
            }
        }
    }

    pub(super) fn lsp_connected(&mut self, sender: UnboundedSender<Event>) {
        self.lsp.sender = Some(sender);
    }

    /// Olvida los servidores elegidos (tras cambiar la configuración) y los que fallaron.
    pub(super) fn restart_lsp(&mut self) {
        self.stop_lsp();
        self.show_info("Servidores de lenguaje reiniciados".to_owned());
    }

    // --- Peticiones -------------------------------------------------------------------

    /// Documento activo sincronizado, su servidor (si está listo) y la posición del cursor.
    fn lsp_target(&mut self) -> Option<(DocumentId, ServerId, PathBuf, Position, usize)> {
        let id = self.workspace.active_id()?;
        self.sync_lsp_document(id);
        let doc = self.lsp.documents.get(&id)?;
        let client = self.lsp_client(doc.server)?;
        if !client.is_ready() {
            return None;
        }
        let document = self.workspace.document(id)?;
        let head = document.editor.selection().head;
        let position = to_position(document.editor.buffer(), head, client.encoding());
        Some((id, doc.server, doc.path.clone(), position, head))
    }

    fn no_server_notice(&mut self) {
        let text = match self.workspace.active() {
            Some(document) if document.path().is_none() => {
                "Guarda el archivo para usar el servidor de lenguaje".to_owned()
            }
            Some(document) => match self.lsp_status() {
                Some((name, false)) => format!("«{name}» aún se está iniciando"),
                _ => format!(
                    "No hay servidor de lenguaje para {} (configúralo en [lsp.servers] de config.toml)",
                    document.language().name()
                ),
            },
            None => return,
        };
        self.show_info(text);
    }

    pub(super) fn request_hover(&mut self) {
        self.lsp.hover = None;
        let Some((document, server, path, position, _)) = self.lsp_target() else {
            self.no_server_notice();
            return;
        };
        let id = self
            .lsp_client(server)
            .and_then(|client| client.hover(&path, position));
        match id {
            Some(id) => {
                self.lsp.hover_request = Some(Request {
                    server,
                    id,
                    document,
                    anchor: 0,
                });
            }
            None => self.show_info("El servidor no ofrece información".to_owned()),
        }
    }

    pub(super) fn request_definition(&mut self) {
        let Some((document, server, path, position, _)) = self.lsp_target() else {
            self.no_server_notice();
            return;
        };
        let id = self
            .lsp_client(server)
            .and_then(|client| client.definition(&path, position));
        match id {
            Some(id) => {
                self.lsp.definition_request = Some(Request {
                    server,
                    id,
                    document,
                    anchor: 0,
                });
            }
            None => self.show_info("El servidor no ofrece «Ir a la definición»".to_owned()),
        }
    }

    /// Pide autocompletado en el cursor. `manual` si lo pidió el usuario (⌃Espacio).
    pub(super) fn request_completion(&mut self, trigger: Option<String>, manual: bool) {
        let Some((document, server, path, position, head)) = self.lsp_target() else {
            if manual {
                self.no_server_notice();
            }
            return;
        };
        let anchor = match self.workspace.document(document) {
            Some(d) if trigger.is_none() => word_start(d.editor.buffer(), head),
            _ => head,
        };
        let id = self
            .lsp_client(server)
            .and_then(|client| client.completion(&path, position, trigger.as_deref()));
        if let Some(id) = id {
            self.lsp.completion_request = Some(Request {
                server,
                id,
                document,
                anchor,
            });
        } else if manual {
            self.show_info("El servidor no ofrece autocompletado".to_owned());
        }
    }

    fn completion_arrived(&mut self, server: ServerId, request: u64, items: Vec<CompletionItem>) {
        let Some(pending) = self.lsp.completion_request else {
            return;
        };
        if pending.server != server || pending.id != request {
            return;
        }
        self.lsp.completion_request = None;
        if items.is_empty() || self.workspace.active_id() != Some(pending.document) {
            return;
        }
        self.lsp.completion = Some(Completion {
            document: pending.document,
            server,
            anchor: pending.anchor,
            items,
            visible: Vec::new(),
            selected: 0,
        });
        self.refilter_completion();
    }

    /// Vuelve a filtrar el desplegable con lo escrito desde el inicio de la palabra; lo
    /// cierra si el cursor salió de ella o ya no encaja nada.
    fn refilter_completion(&mut self) {
        let Some(completion) = &self.lsp.completion else {
            return;
        };
        let Some(document) = self.workspace.document(completion.document) else {
            self.lsp.completion = None;
            return;
        };
        let editor = &document.editor;
        let head = editor.selection().head;
        let buffer = editor.buffer();
        let valid = editor.selections().len() == 1
            && editor.selection().is_empty()
            && head >= completion.anchor
            && buffer.char_to_line(head) == buffer.char_to_line(completion.anchor);
        let prefix = if valid {
            buffer.slice(completion.anchor..head).ok()
        } else {
            None
        };
        let Some(prefix) = prefix.filter(|p| p.chars().all(is_identifier)) else {
            self.lsp.completion = None;
            return;
        };
        let visible = filter_completions(&completion.items, &prefix);
        if visible.is_empty() {
            self.lsp.completion = None;
            return;
        }
        if let Some(completion) = &mut self.lsp.completion {
            completion.visible = visible;
            completion.selected = 0;
        }
    }

    /// Tras una acción del editor: abre, filtra o cierra los desplegables.
    pub(super) fn after_editor_action(
        &mut self,
        id: DocumentId,
        action: &crate::editor::EditorAction,
    ) {
        use crate::editor::EditorAction as A;
        match action {
            A::Viewport(_) | A::CursorPoint(_) | A::Completion(_) | A::Scroll { .. } => return,
            _ => {}
        }
        self.lsp.hover = None;
        let typed = match action {
            A::Insert(text) => Some(text.as_str()),
            A::Backspace | A::DeleteWordBackward => {
                self.refilter_completion();
                return;
            }
            _ => None,
        };
        let Some(typed) = typed else {
            self.lsp.completion = None;
            self.lsp.completion_request = None;
            return;
        };
        if self
            .lsp
            .completion
            .as_ref()
            .is_some_and(|c| c.document == id)
        {
            self.refilter_completion();
            if self.lsp.completion.is_some() {
                return;
            }
        }
        let Some(doc) = self.lsp.documents.get(&id) else {
            return;
        };
        let triggers = self
            .lsp_client(doc.server)
            .map(|c| c.capabilities().trigger_characters)
            .unwrap_or_default();
        let last = typed.chars().last();
        if let Some(trigger) = triggers.iter().find(|t| typed.ends_with(t.as_str())) {
            let trigger = trigger.clone();
            self.request_completion(Some(trigger), false);
            return;
        }
        if !self.config.lsp.auto_completion || !last.is_some_and(is_identifier) {
            return;
        }
        let Some(document) = self.workspace.document(id) else {
            return;
        };
        let head = document.editor.selection().head;
        if document.editor.selections().len() == 1
            && head - word_start(document.editor.buffer(), head) >= AUTO_COMPLETE_AFTER
            && self.lsp.completion_request.is_none()
        {
            self.request_completion(None, false);
        }
    }

    /// ↑, ↓ e Intro/Tab con el desplegable abierto.
    pub(super) fn completion_key(&mut self, key: CompletionKey) {
        let Some(completion) = &mut self.lsp.completion else {
            return;
        };
        let count = completion.visible.len();
        match key {
            CompletionKey::Up => {
                completion.selected = (completion.selected + count - 1) % count.max(1);
            }
            CompletionKey::Down => completion.selected = (completion.selected + 1) % count.max(1),
            CompletionKey::Accept => {
                let selected = completion.selected;
                self.accept_completion(selected);
            }
        }
    }

    /// Inserta la propuesta `row` (posición en la lista visible).
    pub(super) fn accept_completion(&mut self, row: usize) {
        let Some(completion) = self.lsp.completion.take() else {
            return;
        };
        let Some(item) = completion
            .visible
            .get(row)
            .and_then(|&index| completion.items.get(index))
        else {
            return;
        };
        let encoding = self
            .lsp_client(completion.server)
            .map(Client::encoding)
            .unwrap_or_default();
        let Some(document) = self.workspace.document_mut(completion.document) else {
            return;
        };
        let editor = &mut document.editor;
        let head = editor.selection().head;
        let buffer = editor.buffer();
        // El rango del servidor se calculó antes de lo último que se tecleó: su final se
        // alarga hasta el cursor.
        let range = match item.range {
            Some(range) => {
                let start = to_index(buffer, range.start, encoding);
                start.min(head)..to_index(buffer, range.end, encoding).max(head)
            }
            None => completion.anchor..head,
        };
        let mut replacements: Vec<(Range<usize>, String)> = item
            .additional_edits
            .iter()
            .map(|edit| {
                let start = to_index(buffer, edit.range.start, encoding);
                let end = to_index(buffer, edit.range.end, encoding).max(start);
                (start..end, edit.new_text.clone())
            })
            .filter(|(r, _)| r.end <= range.start || r.start >= range.end)
            .collect();
        replacements.push((range.clone(), item.text.clone()));
        replacements.sort_by_key(|(r, _)| r.start);
        // Desplazamiento que causan las ediciones extra anteriores al rango principal.
        let shift: isize = replacements
            .iter()
            .filter(|(r, _)| r.end <= range.start && *r != range)
            .map(|(r, text)| text.chars().count() as isize - r.len() as isize)
            .sum();
        let cursor = item.cursor.unwrap_or_else(|| item.text.chars().count());
        match editor.replace_ranges(&replacements) {
            Ok(_) => {
                let target = range.start.saturating_add_signed(shift) + cursor;
                editor.set_cursor(target, false);
                document.refresh_language();
                self.reveal_active_cursor();
            }
            Err(error) => self.show_error(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str, sort: &str) -> CompletionItem {
        CompletionItem {
            label: label.to_owned(),
            detail: None,
            kind: "",
            filter_text: label.to_owned(),
            sort_text: sort.to_owned(),
            text: label.to_owned(),
            cursor: None,
            range: None,
            additional_edits: Vec::new(),
        }
    }

    #[test]
    fn completions_filter_fuzzily_and_keep_server_order() {
        let items = vec![
            item("verde_viento", "2"),
            item("verde", "1"),
            item("luna", "0"),
        ];
        assert_eq!(filter_completions(&items, ""), vec![2, 1, 0]);
        let found = filter_completions(&items, "ver");
        assert_eq!(found.len(), 2);
        assert!(!found.contains(&2));
        assert_eq!(filter_completions(&items, "vv"), vec![0]);
    }

    #[test]
    fn word_start_stops_at_non_identifiers() {
        let buffer = Buffer::from_text("let ñandú = luna.ver");
        let end = buffer.len_chars();
        assert_eq!(word_start(&buffer, end), end - 3);
        assert_eq!(word_start(&buffer, 9), 4);
        assert_eq!(word_start(&buffer, 10), 10);
        assert_eq!(word_start(&buffer, 0), 0);
    }

    #[test]
    fn positions_convert_both_ways() {
        let buffer = Buffer::from_text("a\n🌙b c");
        let index = buffer.len_chars() - 1;
        let utf16 = to_position(&buffer, index, PositionEncoding::Utf16);
        assert_eq!(utf16, Position::new(1, 4));
        assert_eq!(to_index(&buffer, utf16, PositionEncoding::Utf16), index);
        let utf32 = to_position(&buffer, index, PositionEncoding::Utf32);
        assert_eq!(utf32, Position::new(1, 3));
        // Líneas fuera del documento: la última.
        assert_eq!(
            to_index(&buffer, Position::new(9, 0), PositionEncoding::Utf32),
            2
        );
    }
}
