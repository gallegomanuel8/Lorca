//! Insertar imágenes en el texto con formato: con el botón de la barra (selector de
//! archivos), pegando (imagen o archivos de imagen copiados) o arrastrando archivos a la
//! ventana. Las imágenes se guardan junto al documento (`lorca_workspace::import_image`,
//! `save_png`), así que un documento sin ruta se guarda antes (`Then::InsertImages`).
//!
//! Es un submódulo de `app` para poder usar su estado privado.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use lorca_core::Selection;
use lorca_formats::insert_images;
use lorca_workspace::{DocumentId, image_alt, image_target, import_image, is_image_path, save_png};

use super::{Lorca, Message, Then, is_prose};
use crate::dialogs;

/// Imagen por insertar.
#[derive(Debug, Clone)]
pub enum ImageSource {
    /// Archivo de imagen (elegido, arrastrado o copiado).
    File(PathBuf),
    /// Imagen del portapapeles (RGBA, 8 bits por canal).
    Pixels {
        width: u32,
        height: u32,
        rgba: Arc<Vec<u8>>,
    },
}

impl Lorca {
    /// Botón «Insertar imagen»: pide las imágenes con el selector de archivos.
    pub(super) fn pick_images(&mut self) -> Task<Message> {
        if self.dialog_open {
            return Task::none();
        }
        let Some(document) = self.workspace.active() else {
            return Task::none();
        };
        if !is_prose(document.language()) {
            self.show_info("Las imágenes son para texto y Markdown".to_owned());
            return Task::none();
        }
        let id = document.id();
        let directory = document.path().and_then(Path::parent).map(PathBuf::from);
        self.dialog_open = true;
        Task::perform(dialogs::pick_images(directory), move |paths| {
            Message::ImagesPicked(id, paths)
        })
    }

    pub(super) fn images_picked(&mut self, id: DocumentId, paths: Vec<PathBuf>) -> Task<Message> {
        self.dialog_open = false;
        let sources = paths.into_iter().map(ImageSource::File).collect();
        self.insert_images(id, sources)
    }

    /// Pegar sin texto en el portapapeles: inserta la imagen copiada, si la hay.
    pub(super) fn paste_image(&mut self, id: DocumentId) -> Task<Message> {
        let prose = self
            .workspace
            .document(id)
            .is_some_and(|document| is_prose(document.language()));
        if !prose {
            return Task::none();
        }
        let files = clipboard_image_files();
        if !files.is_empty() {
            return self.insert_images(id, files.into_iter().map(ImageSource::File).collect());
        }
        let image = arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_image());
        let Ok(image) = image else {
            return Task::none();
        };
        let (Ok(width), Ok(height)) = (u32::try_from(image.width), u32::try_from(image.height))
        else {
            return Task::none();
        };
        let source = ImageSource::Pixels {
            width,
            height,
            rgba: Arc::new(image.bytes.into_owned()),
        };
        self.insert_images(id, vec![source])
    }

    /// Pegar texto en un documento con formato: si lo copiado son archivos de imagen (en
    /// el Finder o el Explorador el texto es su nombre), se insertan las imágenes.
    /// Devuelve `None` si hay que pegar el texto.
    pub(super) fn paste_image_files(&mut self, id: DocumentId) -> Option<Task<Message>> {
        let prose = self
            .workspace
            .document(id)
            .is_some_and(|document| is_prose(document.language()));
        if !prose {
            return None;
        }
        let files = clipboard_image_files();
        if files.is_empty() {
            return None;
        }
        Some(self.insert_images(id, files.into_iter().map(ImageSource::File).collect()))
    }

    /// Archivo soltado en la ventana: una imagen se inserta en el documento activo (si es
    /// de texto); cualquier otro archivo se abre.
    pub(super) fn file_dropped(&mut self, path: PathBuf) -> Task<Message> {
        let target = self
            .workspace
            .active()
            .filter(|document| is_prose(document.language()))
            .map(|document| document.id());
        match target {
            Some(id) if is_image_path(&path) => {
                self.insert_images(id, vec![ImageSource::File(path)])
            }
            _ => {
                self.open_paths(&[path]);
                Task::none()
            }
        }
    }

    /// Guarda las imágenes junto al documento `id` y las enlaza donde está el cursor. Si
    /// el documento no tiene ruta, primero se guarda (y luego se sigue con ellas).
    pub(super) fn insert_images(
        &mut self,
        id: DocumentId,
        sources: Vec<ImageSource>,
    ) -> Task<Message> {
        let sources: Vec<ImageSource> = sources
            .into_iter()
            .filter(|source| match source {
                ImageSource::File(path) => is_image_path(path),
                ImageSource::Pixels { .. } => true,
            })
            .collect();
        if sources.is_empty() {
            return Task::none();
        }
        let Some(document) = self.workspace.document(id) else {
            return Task::none();
        };
        if !is_prose(document.language()) {
            self.show_info("Las imágenes son para texto y Markdown".to_owned());
            return Task::none();
        }
        let Some(path) = document.path().map(Path::to_path_buf) else {
            // Sin ruta no hay carpeta donde dejarlas: se guarda antes. Lo que llegue
            // mientras está abierto el diálogo se añade a lo pendiente.
            match &mut self.pending_images {
                Some((pending_id, pending)) if *pending_id == id => pending.extend(sources),
                _ => self.pending_images = Some((id, sources)),
            }
            if self.dialog_open {
                return Task::none();
            }
            self.show_info(
                "Guarda el documento: las imágenes se guardan en una carpeta junto a él".to_owned(),
            );
            return self.save_as(id, Then::InsertImages);
        };

        let mut images = Vec::new();
        let mut errors = Vec::new();
        for source in sources {
            let stored = match source {
                ImageSource::File(file) => import_image(&path, &file),
                ImageSource::Pixels {
                    width,
                    height,
                    rgba,
                } => save_png(&path, width, height, &rgba),
            };
            match stored {
                Ok(image) => images.push((image_alt(&image), image_target(&path, &image))),
                Err(error) => errors.push(error.to_string()),
            }
        }
        if !errors.is_empty() {
            self.show_error(errors.join(" · "));
        }
        if images.is_empty() {
            return Task::none();
        }
        self.workspace.activate(id);
        let Some(document) = self.workspace.document_mut(id) else {
            return Task::none();
        };
        let edit = insert_images(
            &document.editor.text(),
            document.editor.selection().range(),
            &images,
        );
        if let Err(error) = document.editor.replace_ranges(&edit.replacements) {
            self.show_error(error.to_string());
            return Task::none();
        }
        document
            .editor
            .set_selection(Selection::new(edit.selection.start, edit.selection.end));
        // Un texto plano con imágenes pasa a ser texto con formato.
        document.adopt_markdown();
        let view = self.views.entry(id).or_default();
        let viewport = view.viewport;
        view.reveal_cursor(&document.editor, viewport);
        Task::none()
    }

    /// Tras guardar un documento sin ruta para insertarle imágenes, sigue con ellas.
    pub(super) fn insert_pending_images(&mut self, id: DocumentId) -> Task<Message> {
        match self.pending_images.take() {
            Some((pending_id, sources)) if pending_id == id => self.insert_images(id, sources),
            _ => Task::none(),
        }
    }
}

/// Archivos de imagen copiados en el portapapeles (en el Finder, el Explorador…).
fn clipboard_image_files() -> Vec<PathBuf> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get().file_list())
        .map(|files| files.into_iter().filter(|f| is_image_path(f)).collect())
        .unwrap_or_default()
}
