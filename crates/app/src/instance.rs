//! Instancia única en la aplicación: arranque y suscripción con los archivos que mandan
//! las ejecuciones siguientes (ver `lorca_workspace::instance`).

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use iced::Subscription;
use iced::futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use iced::futures::{SinkExt, Stream, StreamExt};
use lorca_workspace::instance::{Launch, claim};

use crate::app::Message;

/// Rutas recibidas de otras ejecuciones, hasta que la suscripción recoge el receptor.
static INCOMING: Mutex<Option<UnboundedReceiver<Vec<PathBuf>>>> = Mutex::new(None);
/// Extremo de envío del mismo canal (también lo usa «Abrir con» de macOS).
static SENDER: OnceLock<UnboundedSender<Vec<PathBuf>>> = OnceLock::new();

/// Entrega `paths` a la ventana como si los hubiera mandado otra ejecución (se abren en
/// pestañas y la ventana pasa al frente). Lo que llega antes de que la suscripción esté
/// lista espera en el canal.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn deliver(paths: Vec<PathBuf>) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.unbounded_send(paths);
    }
}

/// Crea el canal (una vez).
fn channel() -> &'static UnboundedSender<Vec<PathBuf>> {
    SENDER.get_or_init(|| {
        let (sender, receiver) = unbounded();
        if let Ok(mut incoming) = INCOMING.lock() {
            *incoming = Some(receiver);
        }
        sender
    })
}

/// Decide si esta ejecución abre la ventana. `false` si otra instancia ya ha recibido
/// `paths` y esta debe terminar.
pub fn start(paths: &[PathBuf]) -> bool {
    let sender = channel().clone();
    let Some(dir) = lorca_workspace::data_dir() else {
        return true;
    };
    match claim(&dir, paths) {
        Launch::Forwarded => false,
        Launch::Standalone => true,
        Launch::Primary(server) => {
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
