//! Instancia única en la aplicación: arranque y suscripción con los archivos que mandan
//! las ejecuciones siguientes (ver `lorca_workspace::instance`).

use std::path::PathBuf;
use std::sync::Mutex;

use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, unbounded};
use iced::futures::{SinkExt, Stream, StreamExt};
use lorca_workspace::instance::{Launch, claim};

use crate::app::Message;

/// Rutas recibidas de otras ejecuciones, hasta que la suscripción recoge el receptor.
static INCOMING: Mutex<Option<UnboundedReceiver<Vec<PathBuf>>>> = Mutex::new(None);

/// Decide si esta ejecución abre la ventana. `false` si otra instancia ya ha recibido
/// `paths` y esta debe terminar.
pub fn start(paths: &[PathBuf]) -> bool {
    let Some(dir) = lorca_workspace::data_dir() else {
        return true;
    };
    match claim(&dir, paths) {
        Launch::Forwarded => false,
        Launch::Standalone => true,
        Launch::Primary(server) => {
            let (sender, receiver) = unbounded();
            if let Ok(mut incoming) = INCOMING.lock() {
                *incoming = Some(receiver);
            }
            server.serve(move |paths| {
                let _ = sender.unbounded_send(paths);
            });
            true
        }
    }
}

/// `Message::OpenedElsewhere` con los archivos de cada ejecución nueva.
pub fn events() -> Subscription<Message> {
    Subscription::run(stream)
}

fn stream() -> impl Stream<Item = Message> {
    iced::stream::channel(8, async |mut output| {
        let receiver = INCOMING
            .lock()
            .ok()
            .and_then(|mut incoming| incoming.take());
        let Some(mut receiver) = receiver else {
            return;
        };
        while let Some(paths) = receiver.next().await {
            if output.send(Message::OpenedElsewhere(paths)).await.is_err() {
                break;
            }
        }
    })
}
