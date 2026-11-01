//! El cliente frente a un servidor falso conectado por tuberías en memoria.

// Las funciones auxiliares de las pruebas no son `#[test]`: se permite `unwrap` en todo
// el archivo.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use lorca_lsp::{Client, Event, Position, PositionEncoding, ServerId, Severity, path_to_uri};
use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(5);

fn write(writer: &mut impl Write, message: &Value) {
    let body = serde_json::to_vec(message).unwrap();
    write!(writer, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    writer.write_all(&body).unwrap();
    writer.flush().unwrap();
}

fn read(reader: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap() == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).unwrap();
    Some(serde_json::from_slice(&body).unwrap())
}

/// Servidor que contesta lo justo y devuelve los métodos recibidos, en orden.
fn fake_server(input: impl Read, mut output: impl Write) -> Vec<String> {
    let mut input = BufReader::new(input);
    let mut methods = Vec::new();
    while let Some(message) = read(&mut input) {
        let method = message["method"].as_str().map(str::to_owned);
        let id = message.get("id").cloned();
        if let Some(method) = &method {
            methods.push(method.clone());
        } else {
            // Respuesta del cliente a nuestra petición de configuración.
            assert_eq!(message["id"], json!("config-1"));
            assert_eq!(message["result"], json!([null]));
            methods.push("(respuesta)".to_owned());
            continue;
        }
        match method.as_deref() {
            Some("initialize") => {
                // Antes de contestar, una petición del servidor: el cliente debe responder.
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": "config-1", "method": "workspace/configuration",
                        "params": {"items": [{"section": "lorca"}]}}),
                );
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": id, "result": {"capabilities": {
                        "positionEncoding": "utf-32",
                        "textDocumentSync": 1,
                        "completionProvider": {"triggerCharacters": ["."]},
                        "hoverProvider": true,
                        "definitionProvider": true,
                    }}}),
                );
            }
            Some("textDocument/didOpen") => {
                let uri = message["params"]["textDocument"]["uri"].clone();
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "method": "textDocument/publishDiagnostics",
                    "params": {"uri": uri, "diagnostics": [{
                        "range": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 7}},
                        "severity": 2,
                        "message": "variable sin usar",
                        "source": "falso",
                    }]}}),
                );
            }
            Some("textDocument/completion") => {
                assert_eq!(message["params"]["context"]["triggerCharacter"], json!("."));
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": id, "result": {"isIncomplete": false, "items": [
                        {"label": "luna", "kind": 6},
                        {"label": "verde()", "insertText": "verde($1)", "insertTextFormat": 2},
                    ]}}),
                );
            }
            Some("textDocument/hover") => {
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": id, "result": {
                        "contents": {"kind": "markdown", "value": "**luna**: `Luna`"}}}),
                );
            }
            Some("textDocument/definition") => {
                let uri = message["params"]["textDocument"]["uri"].clone();
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": id, "result": {"uri": uri,
                        "range": {"start": {"line": 4, "character": 0}, "end": {"line": 4, "character": 4}}}}),
                );
            }
            Some("shutdown") => {
                write(
                    &mut output,
                    &json!({"jsonrpc": "2.0", "id": id, "result": null}),
                );
            }
            Some("exit") => break,
            _ => {}
        }
    }
    methods
}

fn next(events: &mpsc::Receiver<Event>) -> Event {
    events
        .recv_timeout(TIMEOUT)
        .expect("el servidor no contestó")
}

#[test]
fn full_conversation() {
    let (client_reads, server_writes) = std::io::pipe().unwrap();
    let (server_reads, client_writes) = std::io::pipe().unwrap();
    let server = std::thread::spawn(move || fake_server(server_reads, server_writes));

    let (sender, events) = mpsc::channel();
    let root = if cfg!(windows) {
        PathBuf::from(r"C:\proyecto")
    } else {
        PathBuf::from("/proyecto")
    };
    let file = root.join("main.rs");
    let id = ServerId(7);
    let mut client = Client::connect(id, client_reads, client_writes, &root, move |event| {
        let _ = sender.send(event);
    });

    // Se manda antes de que el servidor esté listo: debe esperar a `initialized`.
    client.did_open(&file, "rust", 1, "let luna = 1;");
    assert_eq!(next(&events), Event::Ready { server: id });
    assert!(client.is_ready());
    assert_eq!(client.encoding(), PositionEncoding::Utf32);
    assert_eq!(
        client.capabilities().trigger_characters,
        vec![".".to_owned()]
    );

    match next(&events) {
        Event::Diagnostics {
            server,
            path,
            diagnostics,
        } => {
            assert_eq!(server, id);
            assert_eq!(path, file);
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].severity, Severity::Warning);
            assert_eq!(diagnostics[0].message, "variable sin usar");
        }
        other => panic!("se esperaban diagnósticos: {other:?}"),
    }

    let position = Position::new(0, 4);
    let request = client.completion(&file, position, Some(".")).unwrap();
    match next(&events) {
        Event::Completion {
            request: got,
            items,
            ..
        } => {
            assert_eq!(got, request);
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].kind, "var");
            assert_eq!(items[1].text, "verde()");
            assert_eq!(items[1].cursor, Some(6));
        }
        other => panic!("se esperaba autocompletado: {other:?}"),
    }

    let request = client.hover(&file, position).unwrap();
    assert_eq!(
        next(&events),
        Event::Hover {
            server: id,
            request,
            text: Some("**luna**: `Luna`".to_owned())
        }
    );

    let request = client.definition(&file, position).unwrap();
    match next(&events) {
        Event::Definition {
            request: got,
            locations,
            ..
        } => {
            assert_eq!(got, request);
            assert_eq!(locations[0].path, file);
            assert_eq!(locations[0].range.start.line, 4);
        }
        other => panic!("se esperaba definición: {other:?}"),
    }

    client.did_change(&file, 2, "let luna = 2;");
    client.shutdown();
    let methods = server.join().unwrap();
    assert!(matches!(next(&events), Event::Exited { server, .. } if server == id));
    assert_eq!(
        methods,
        [
            "initialize",
            "(respuesta)",
            "initialized",
            "textDocument/didOpen",
            "textDocument/completion",
            "textDocument/hover",
            "textDocument/definition",
            "textDocument/didChange",
            "shutdown",
            "exit",
        ]
    );
    assert!(path_to_uri(Path::new(&file)).is_some());
}
