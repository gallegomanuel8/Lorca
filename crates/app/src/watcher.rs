//! Vigilancia de los archivos abiertos para detectar cambios hechos por otros programas.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::Subscription;
use iced::futures::{SinkExt, Stream, StreamExt};
use notify::{EventKind, RecursiveMode, Watcher};

use crate::app::Message;

/// Qué se vigila: archivos concretos y carpetas enteras (cualquier archivo de dentro).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Watched {
    pub files: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
}

/// Suscripción que avisa con `Message::FileChanged` cuando cambia alguno de los archivos
/// o algún archivo directamente dentro de las carpetas.
///
/// Para los archivos se vigila su carpeta, porque muchos programas guardan escribiendo
/// un temporal y renombrándolo, lo que rompe la vigilancia del archivo original. Si la
/// lista cambia, iced reinicia la suscripción con la lista nueva.
pub fn watch(mut watched: Watched) -> Subscription<Message> {
    if watched.files.is_empty() && watched.folders.is_empty() {
        return Subscription::none();
    }
    watched.files.sort();
    watched.files.dedup();
    watched.folders.sort();
    watched.folders.dedup();
    Subscription::run_with(watched, stream)
}

fn stream(watched: &Watched) -> impl Stream<Item = Message> + use<> {
    let paths = watched.files.clone();
    let watched_folders = watched.folders.clone();
    iced::stream::channel(64, async move |mut output| {
        let (sender, mut receiver) = iced::futures::channel::mpsc::unbounded::<PathBuf>();
        let watched: HashSet<PathBuf> = paths.iter().cloned().collect();
        let folder_set: HashSet<PathBuf> = watched_folders.iter().cloned().collect();
        let handler = move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if !matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                let in_folder = path.parent().is_some_and(|p| folder_set.contains(p));
                if watched.contains(&path) || in_folder {
                    let _ = sender.unbounded_send(path);
                }
            }
        };
        let Ok(mut watcher) = notify::recommended_watcher(handler) else {
            return;
        };
        let folders: HashSet<&Path> = paths
            .iter()
            .filter_map(|p| p.parent())
            .chain(watched_folders.iter().map(PathBuf::as_path))
            .collect();
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
