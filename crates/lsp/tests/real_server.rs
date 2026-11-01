//! Prueba con un servidor real (clangd) si está instalado; si no, no hace nada.

// Las funciones auxiliares de las pruebas no son `#[test]`: se permite `unwrap` en todo
// el archivo.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::mpsc;
use std::time::Duration;

use lorca_lsp::{Client, Event, Position, ServerCommand, ServerId, servers};

#[test]
fn clangd_reports_diagnostics_and_hover() {
    if servers::find_executable("clangd").is_none() {
        eprintln!("clangd no está instalado: se omite");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("romance.c");
    let text = "int luna(void) { return 1; }\nint main(void) { return luna() + noche; }\n";
    std::fs::write(&file, text).unwrap();

    let (sender, events) = mpsc::channel();
    let client = Client::spawn(
        ServerId(1),
        &ServerCommand::new("clangd", &[]),
        dir.path(),
        move |event| {
            let _ = sender.send(event);
        },
    )
    .unwrap();
    client.did_open(&file, "c", 1, text);

    let wait = Duration::from_secs(30);
    let mut ready = false;
    let mut diagnostics = None;
    while diagnostics.is_none() {
        match events.recv_timeout(wait).expect("clangd no contestó") {
            Event::Ready { .. } => ready = true,
            Event::Diagnostics { diagnostics: d, .. } if !d.is_empty() => diagnostics = Some(d),
            Event::Exited { message, .. } => panic!("clangd terminó: {message}"),
            _ => {}
        }
    }
    assert!(ready);
    let diagnostics = diagnostics.unwrap();
    assert!(
        diagnostics.iter().any(|d| d.message.contains("noche")),
        "{diagnostics:?}"
    );

    let request = client.hover(&file, Position::new(0, 5)).unwrap();
    loop {
        match events.recv_timeout(wait).expect("sin respuesta de hover") {
            Event::Hover {
                request: got, text, ..
            } if got == request => {
                let text = text.unwrap_or_default();
                assert!(text.contains("luna"), "{text:?}");
                break;
            }
            _ => {}
        }
    }
}
