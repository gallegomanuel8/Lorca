//! Vigilancia de los archivos abiertos para detectar cambios hechos por otros programas.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::app::Message;

/// Suscripción que avisa con `Message::FileChanged` cuando cambia alguno de `paths`.
///
/// Se vigilan las carpetas (no los archivos), porque muchos programas guardan escribiendo
/// un temporal y renombrándolo, lo que rompe la vigilancia del archivo original. Si la
/// lista de rutas cambia, iced reinicia la suscripción con la lista nueva.
pub fn watch(mut paths: Vec<PathBuf>) -> Subscription<Message> {
    if paths.is_empty() {
        return Subscription::none();
    }
    paths.sort();
    paths.dedup();
    Subscription::run_with(paths, stream)
}

#[allow(clippy::ptr_arg)] // `Subscription::run_with` exige `fn(&D)` con `D = Vec<PathBuf>`.
fn stream(paths: &Vec<PathBuf>) -> impl Stream<Item = Message> + use<> {
    let paths = paths.clone();
    iced::stream::channel(64, async move |mut output| {
        let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded::<PathBuf>();
        let watched: HashSet<PathBuf> = paths.iter().cloned().collect();
        let handler = move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if !matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                if watched.contains(&path) {
                    let _ = sender.unbounded_send(path);
                }
            }
        };
        let Ok(mut watcher) = notify::recommended_watcher(handler) else {
            return;
        };
        let folders: HashSet<&Path> = paths.iter().filter_map(|p| p.parent()).collect();
        for folder in folders {
            // Una carpeta que ya no existe no impide vigilar las demás.
            let _ = watcher.watch(folder, RecursiveMode::NonRecursive);
        }
        while let Some(path) = receiver.next().await {
            if output.send(Message::FileChanged(path)).await.is_err() {
                break;
            }
        }
        drop(watcher);
    })
}
