//! Cliente de un servidor de lenguaje: proceso hijo, hilos de lectura y escritura y
//! correspondencia entre peticiones y respuestas.
//!
//! La aplicación llama a los métodos (`did_open`, `completion`…) desde su hilo y recibe
//! las respuestas como [`Event`] a través de la función que pasa al crear el cliente.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use lsp_types::{
    CompletionResponse, GotoDefinitionResponse, Hover, InitializeResult, PublishDiagnosticsParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind,
    TextDocumentSyncSaveOptions,
};
use serde_json::{Value, json};

use crate::framing::{read_message, write_message};
use crate::servers::{ServerCommand, find_executable, search_path};
use crate::types::{
    Event, Position, ServerId, completion_item, diagnostic, hover_text, locations, path_to_uri,
    uri_to_path,
};
use crate::{LspError, PositionEncoding};

/// Líneas de la salida de errores del servidor que se guardan para explicar un cierre.
const STDERR_LINES: usize = 20;
/// Cuánto se espera a que el servidor salga por sí mismo antes de matarlo.
const EXIT_GRACE: Duration = Duration::from_secs(2);

type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Qué se esperaba de cada petición en curso.
#[derive(Debug, Clone, Copy)]
enum Pending {
    Initialize,
    Completion,
    Hover,
    Definition,
    Shutdown,
}

/// Lo que se manda al hilo de escritura.
#[derive(Debug)]
enum Outgoing {
    /// `initialize`: es lo único que se envía antes de que el servidor conteste.
    Initialize(Value),
    /// Peticiones y notificaciones: esperan a que el servidor esté listo.
    Message(Value),
    /// Respuestas a peticiones del servidor: salen enseguida.
    Response(Value),
    /// El servidor contestó a `initialize`: se envía `initialized` y lo que esperaba.
    Ready,
    Stop,
}

/// Capacidades del servidor que le interesan a la aplicación.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub completion: bool,
    pub trigger_characters: Vec<String>,
    pub hover: bool,
    pub definition: bool,
    /// Enviar `didChange` (el servidor no pidió `None`).
    sync_changes: bool,
    /// Incluir el texto en `didSave`.
    save_text: bool,
}

impl Capabilities {
    fn from_server(caps: &ServerCapabilities) -> Self {
        let (sync_changes, save_text) = match &caps.text_document_sync {
            Some(TextDocumentSyncCapability::Kind(kind)) => {
                (*kind != TextDocumentSyncKind::NONE, false)
            }
            Some(TextDocumentSyncCapability::Options(options)) => {
                let changes = options
                    .change
                    .is_none_or(|kind| kind != TextDocumentSyncKind::NONE);
                let save_text = matches!(
                    &options.save,
                    Some(TextDocumentSyncSaveOptions::SaveOptions(save))
                        if save.include_text == Some(true)
                );
                (changes, save_text)
            }
            None => (true, false),
        };
        let enabled = |option: &Option<lsp_types::OneOf<bool, _>>| {
            matches!(
                option,
                Some(lsp_types::OneOf::Left(true) | lsp_types::OneOf::Right(_))
            )
        };
        Self {
            completion: caps.completion_provider.is_some(),
            trigger_characters: caps
                .completion_provider
                .as_ref()
                .and_then(|c| c.trigger_characters.clone())
                .unwrap_or_default(),
            hover: !matches!(
                caps.hover_provider,
                None | Some(lsp_types::HoverProviderCapability::Simple(false))
            ),
            definition: enabled(&caps.definition_provider),
            sync_changes,
            save_text,
        }
    }
}

#[derive(Debug, Default)]
struct State {
    ready: bool,
    encoding: PositionEncoding,
    capabilities: Capabilities,
}

/// Estado compartido entre el cliente y sus hilos.
#[derive(Debug, Default)]
struct Shared {
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Pending>>,
    state: Mutex<State>,
    stderr: Mutex<VecDeque<String>>,
}

/// Conexión con un servidor de lenguaje.
pub struct Client {
    id: ServerId,
    outgoing: mpsc::Sender<Outgoing>,
    shared: Arc<Shared>,
    child: Option<Child>,
    stopped: bool,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("id", &self.id)
            .field("ready", &self.is_ready())
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Lanza el servidor y empieza la inicialización para la carpeta `root`.
    pub fn spawn(
        id: ServerId,
        command: &ServerCommand,
        root: &Path,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Result<Self, LspError> {
        let spawn_error = |source| LspError::Spawn {
            command: command.command.clone(),
            source,
        };
        let program = find_executable(&command.command).ok_or_else(|| {
            spawn_error(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no está instalado o no está en el PATH",
            ))
        })?;
        let mut child = Command::new(program)
            .args(&command.args)
            .current_dir(root)
            .env("PATH", search_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(spawn_error)?;
        let missing = || spawn_error(std::io::Error::other("sin tuberías de E/S"));
        let stdin = child.stdin.take().ok_or_else(missing)?;
        let stdout = child.stdout.take().ok_or_else(missing)?;
        let stderr = child.stderr.take();
        let mut client = Self::connect(id, stdout, stdin, root, on_event);
        if let Some(stderr) = stderr {
            let shared = Arc::clone(&client.shared);
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let mut tail = lock(&shared.stderr);
                    if tail.len() == STDERR_LINES {
                        tail.pop_front();
                    }
                    tail.push_back(line);
                }
            });
        }
        client.child = Some(child);
        Ok(client)
    }

    /// Cliente sobre un canal ya abierto (la salida y la entrada del servidor).
    pub fn connect(
        id: ServerId,
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        root: &Path,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> Self {
        let shared = Arc::new(Shared::default());
        let (outgoing, receiver) = mpsc::channel();
        std::thread::spawn(move || write_loop(writer, &receiver));
        let sink: EventSink = Arc::new(on_event);
        {
            let shared = Arc::clone(&shared);
            let outgoing = outgoing.clone();
            std::thread::spawn(move || read_loop(id, reader, &shared, &outgoing, &sink));
        }
        let client = Self {
            id,
            outgoing,
            shared,
            child: None,
            stopped: false,
        };
        let request_id = client.register(Pending::Initialize);
        let _ = client.outgoing.send(Outgoing::Initialize(json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "initialize",
            "params": initialize_params(root),
        })));
        client
    }

    pub fn id(&self) -> ServerId {
        self.id
    }

    /// El servidor ya contestó a `initialize`.
    pub fn is_ready(&self) -> bool {
        lock(&self.shared.state).ready
    }

    /// Unidad de las columnas negociada con el servidor.
    pub fn encoding(&self) -> PositionEncoding {
        lock(&self.shared.state).encoding
    }

    pub fn capabilities(&self) -> Capabilities {
        lock(&self.shared.state).capabilities.clone()
    }

    fn register(&self, pending: Pending) -> u64 {
        let id = self.shared.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.shared.pending).insert(id, pending);
        id
    }

    fn notify(&self, method: &str, params: Value) {
        let _ = self.outgoing.send(Outgoing::Message(json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        })));
    }

    fn request(&self, method: &str, params: Value, pending: Pending) -> u64 {
        let id = self.register(pending);
        let _ = self.outgoing.send(Outgoing::Message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })));
        id
    }

    pub fn did_open(&self, path: &Path, language_id: &str, version: i32, text: &str) {
        let Some(uri) = path_to_uri(path) else { return };
        self.notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri,
                "languageId": language_id,
                "version": version,
                "text": text,
            }}),
        );
    }

    /// Envía el texto completo (válido tanto para sincronización completa como
    /// incremental: un cambio sin rango sustituye todo el documento).
    pub fn did_change(&self, path: &Path, version: i32, text: &str) {
        let Some(uri) = path_to_uri(path) else { return };
        if self.is_ready() && !self.capabilities().sync_changes {
            return;
        }
        self.notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri, "version": version},
                "contentChanges": [{"text": text}],
            }),
        );
    }

    pub fn did_save(&self, path: &Path, text: &str) {
        let Some(uri) = path_to_uri(path) else { return };
        let mut params = json!({"textDocument": {"uri": uri}});
        if self.capabilities().save_text {
            params["text"] = json!(text);
        }
        self.notify("textDocument/didSave", params);
    }

    pub fn did_close(&self, path: &Path) {
        let Some(uri) = path_to_uri(path) else { return };
        self.notify(
            "textDocument/didClose",
            json!({"textDocument": {"uri": uri}}),
        );
    }

    fn position_params(path: &Path, position: Position) -> Option<Value> {
        let uri = path_to_uri(path)?;
        Some(json!({"textDocument": {"uri": uri}, "position": position}))
    }

    /// Pide propuestas de autocompletado. `trigger` es el carácter que lo provocó, si lo
    /// hubo. Devuelve el número de petición (`None` si el servidor no lo admite).
    pub fn completion(
        &self,
        path: &Path,
        position: Position,
        trigger: Option<&str>,
    ) -> Option<u64> {
        if !self.capabilities().completion {
            return None;
        }
        let mut params = Self::position_params(path, position)?;
        params["context"] = match trigger {
            Some(c) => json!({"triggerKind": 2, "triggerCharacter": c}),
            None => json!({"triggerKind": 1}),
        };
        Some(self.request("textDocument/completion", params, Pending::Completion))
    }

    pub fn hover(&self, path: &Path, position: Position) -> Option<u64> {
        if !self.capabilities().hover {
            return None;
        }
        let params = Self::position_params(path, position)?;
        Some(self.request("textDocument/hover", params, Pending::Hover))
    }

    pub fn definition(&self, path: &Path, position: Position) -> Option<u64> {
        if !self.capabilities().definition {
            return None;
        }
        let params = Self::position_params(path, position)?;
        Some(self.request("textDocument/definition", params, Pending::Definition))
    }

    /// Pide al servidor que termine (`shutdown` y luego `exit`). Si no sale en unos
    /// segundos, se mata el proceso.
    pub fn shutdown(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        if self.is_ready() {
            self.request("shutdown", Value::Null, Pending::Shutdown);
        } else {
            let _ = self.outgoing.send(Outgoing::Stop);
        }
        if let Some(mut child) = self.child.take() {
            std::thread::spawn(move || {
                let deadline = Instant::now() + EXIT_GRACE;
                while Instant::now() < deadline {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = child.kill();
                let _ = child.wait();
            });
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn initialize_params(root: &Path) -> Value {
    let root_uri = path_to_uri(root);
    let name = root
        .file_name()
        .map_or_else(|| "lorca".to_owned(), |n| n.to_string_lossy().into_owned());
    json!({
        "processId": std::process::id(),
        "clientInfo": {"name": "Lorca", "version": env!("CARGO_PKG_VERSION")},
        "rootUri": root_uri,
        "rootPath": root,
        "workspaceFolders": root_uri.map(|uri| vec![json!({"uri": uri, "name": name})]),
        "capabilities": {
            "general": {"positionEncodings": ["utf-32", "utf-16"]},
            "textDocument": {
                "synchronization": {"didSave": true, "dynamicRegistration": false},
                "completion": {
                    "completionItem": {
                        "snippetSupport": false,
                        "documentationFormat": ["markdown", "plaintext"],
                    },
                    "contextSupport": true,
                },
                "hover": {"contentFormat": ["markdown", "plaintext"]},
                "definition": {"linkSupport": true},
                "publishDiagnostics": {"relatedInformation": false},
            },
            "window": {"workDoneProgress": false},
            "workspace": {"workspaceFolders": true, "configuration": true},
        },
    })
}

fn write_loop(mut writer: impl Write, receiver: &mpsc::Receiver<Outgoing>) {
    let mut ready = false;
    let mut queue = Vec::new();
    let mut send = |message: &Value| write_message(&mut writer, message).is_ok();
    for outgoing in receiver {
        let ok = match outgoing {
            Outgoing::Initialize(message) | Outgoing::Response(message) => send(&message),
            Outgoing::Message(message) if ready => send(&message),
            Outgoing::Message(message) => {
                queue.push(message);
                true
            }
            Outgoing::Ready => {
                ready = true;
                let initialized = json!({"jsonrpc": "2.0", "method": "initialized", "params": {}});
                send(&initialized) && queue.drain(..).all(|message| send(&message))
            }
            Outgoing::Stop => break,
        };
        if !ok {
            break;
        }
    }
}

fn read_loop(
    server: ServerId,
    reader: impl Read,
    shared: &Shared,
    outgoing: &mpsc::Sender<Outgoing>,
    sink: &EventSink,
) {
    let mut reader = BufReader::new(reader);
    let reason = loop {
        match read_message(&mut reader) {
            Ok(Some(message)) => handle_message(server, message, shared, outgoing, sink),
            Ok(None) => break "el servidor cerró la conexión".to_owned(),
            Err(error) => break error.to_string(),
        }
    };
    let _ = outgoing.send(Outgoing::Stop);
    let tail: Vec<String> = lock(&shared.stderr).iter().cloned().collect();
    let message = match tail.last() {
        Some(last) => format!("{reason}: {last}"),
        None => reason,
    };
    sink(Event::Exited { server, message });
}

fn handle_message(
    server: ServerId,
    message: Value,
    shared: &Shared,
    outgoing: &mpsc::Sender<Outgoing>,
    sink: &EventSink,
) {
    let method = message.get("method").and_then(Value::as_str);
    let id = message.get("id").cloned();
    match (method, id) {
        // Petición del servidor: hay que contestar siempre.
        (Some(method), Some(id)) => {
            let result = server_request_result(method, message.get("params"));
            let response = match result {
                Some(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                None => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {"code": -32601, "message": format!("método no admitido: {method}")},
                }),
            };
            let _ = outgoing.send(Outgoing::Response(response));
        }
        (Some(method), None) => handle_notification(server, method, message.get("params"), sink),
        (None, Some(id)) => {
            let Some(request) = id.as_u64() else { return };
            let Some(pending) = lock(&shared.pending).remove(&request) else {
                return;
            };
            if let Some(error) = message.get("error") {
                let text = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("error desconocido")
                    .to_owned();
                // Peticiones canceladas o desfasadas: no merecen aviso.
                let code = error.get("code").and_then(Value::as_i64);
                if !matches!(code, Some(-32802..=-32800)) {
                    sink(Event::Failed {
                        server,
                        request: Some(request),
                        message: text,
                    });
                }
                return;
            }
            let result = message.get("result").cloned().unwrap_or(Value::Null);
            handle_response(server, request, pending, result, shared, outgoing, sink);
        }
        (None, None) => {}
    }
}

/// Respuesta a las peticiones del servidor que el cliente entiende; `None` = no admitida.
fn server_request_result(method: &str, params: Option<&Value>) -> Option<Value> {
    match method {
        // Sin ajustes propios: `null` para cada sección pedida.
        "workspace/configuration" => {
            let count = params
                .and_then(|p| p.get("items"))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            Some(Value::Array(vec![Value::Null; count]))
        }
        "window/workDoneProgress/create"
        | "client/registerCapability"
        | "client/unregisterCapability"
        | "window/showMessageRequest"
        | "workspace/codeLens/refresh"
        | "workspace/semanticTokens/refresh"
        | "workspace/inlayHint/refresh"
        | "workspace/diagnostic/refresh" => Some(Value::Null),
        "workspace/workspaceFolders" => Some(Value::Null),
        "workspace/applyEdit" => Some(json!({"applied": false})),
        _ => None,
    }
}

fn handle_notification(server: ServerId, method: &str, params: Option<&Value>, sink: &EventSink) {
    let params = params.cloned().unwrap_or(Value::Null);
    match method {
        "textDocument/publishDiagnostics" => {
            let Ok(params) = serde_json::from_value::<PublishDiagnosticsParams>(params) else {
                return;
            };
            let Some(path) = uri_to_path(&params.uri) else {
                return;
            };
            sink(Event::Diagnostics {
                server,
                path,
                diagnostics: params.diagnostics.into_iter().map(diagnostic).collect(),
            });
        }
        "window/showMessage" => {
            let kind = params.get("type").and_then(Value::as_i64).unwrap_or(4);
            let text = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            // Solo errores y avisos; la información y los registros no se muestran.
            if kind <= 2 && !text.is_empty() {
                sink(Event::Message {
                    server,
                    text,
                    is_error: kind == 1,
                });
            }
        }
        _ => {}
    }
}

fn handle_response(
    server: ServerId,
    request: u64,
    pending: Pending,
    result: Value,
    shared: &Shared,
    outgoing: &mpsc::Sender<Outgoing>,
    sink: &EventSink,
) {
    let failed = |message: String| Event::Failed {
        server,
        request: Some(request),
        message,
    };
    match pending {
        Pending::Initialize => {
            let result = match serde_json::from_value::<InitializeResult>(result) {
                Ok(result) => result,
                Err(error) => {
                    sink(failed(format!("respuesta de inicio inválida: {error}")));
                    return;
                }
            };
            {
                let mut state = lock(&shared.state);
                state.ready = true;
                state.encoding = result
                    .capabilities
                    .position_encoding
                    .as_ref()
                    .map(PositionEncoding::from_kind)
                    .unwrap_or_default();
                state.capabilities = Capabilities::from_server(&result.capabilities);
            }
            let _ = outgoing.send(Outgoing::Ready);
            sink(Event::Ready { server });
        }
        Pending::Completion => {
            let items = match serde_json::from_value::<Option<CompletionResponse>>(result) {
                Ok(Some(CompletionResponse::Array(items))) => items,
                Ok(Some(CompletionResponse::List(list))) => list.items,
                Ok(None) => Vec::new(),
                Err(error) => {
                    sink(failed(error.to_string()));
                    return;
                }
            };
            sink(Event::Completion {
                server,
                request,
                items: items.into_iter().map(completion_item).collect(),
            });
        }
        Pending::Hover => {
            let text = match serde_json::from_value::<Option<Hover>>(result) {
                Ok(hover) => hover.and_then(|h| hover_text(h.contents)),
                Err(error) => {
                    sink(failed(error.to_string()));
                    return;
                }
            };
            sink(Event::Hover {
                server,
                request,
                text,
            });
        }
        Pending::Definition => {
            let found = match serde_json::from_value::<Option<GotoDefinitionResponse>>(result) {
                Ok(response) => response.map(locations).unwrap_or_default(),
                Err(error) => {
                    sink(failed(error.to_string()));
                    return;
                }
            };
            sink(Event::Definition {
                server,
                request,
                locations: found,
            });
        }
        Pending::Shutdown => {
            let _ = outgoing.send(Outgoing::Message(
                json!({"jsonrpc": "2.0", "method": "exit"}),
            ));
            let _ = outgoing.send(Outgoing::Stop);
        }
    }
}
