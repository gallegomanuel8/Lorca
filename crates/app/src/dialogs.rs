//! Diálogos nativos (rfd), siempre asíncronos para no bloquear la interfaz.

use std::path::PathBuf;

use rfd::{AsyncFileDialog, AsyncMessageDialog, MessageButtons, MessageDialogResult, MessageLevel};

/// Respuesta a «¿Guardar los cambios?».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveChoice {
    Save,
    Discard,
    Cancel,
}

const SAVE: &str = "Guardar";
const DISCARD: &str = "No guardar";
const CANCEL: &str = "Cancelar";

/// Pide uno o varios archivos para abrir.
pub async fn pick_files_to_open() -> Vec<PathBuf> {
    AsyncFileDialog::new()
        .set_title("Abrir")
        .pick_files()
        .await
        .map(|files| files.iter().map(|f| f.path().to_path_buf()).collect())
        .unwrap_or_default()
}

/// Pide la ruta donde guardar, proponiendo `file_name`.
pub async fn pick_save_path(file_name: String, directory: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = AsyncFileDialog::new()
        .set_title("Guardar como")
        .set_file_name(file_name);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    dialog.save_file().await.map(|f| f.path().to_path_buf())
}

/// Pide dónde exportar un PDF, proponiendo `file_name`.
pub async fn pick_pdf_path(file_name: String, directory: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = AsyncFileDialog::new()
        .set_title("Exportar a PDF")
        .set_file_name(file_name)
        .add_filter("PDF", &["pdf"]);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    dialog.save_file().await.map(|f| f.path().to_path_buf())
}

/// Pregunta si se guardan los cambios del documento `name`.
pub async fn confirm_save(name: String) -> SaveChoice {
    // En Windows los botones personalizados requieren common-controls v6 y un manifiesto;
    // allí se usan Sí/No/Cancelar, que el sistema traduce.
    let buttons = if cfg!(windows) {
        MessageButtons::YesNoCancel
    } else {
        MessageButtons::YesNoCancelCustom(SAVE.into(), DISCARD.into(), CANCEL.into())
    };
    let result = AsyncMessageDialog::new()
        .set_level(MessageLevel::Warning)
        .set_title("Cambios sin guardar")
        .set_description(format!(
            "¿Quieres guardar los cambios de «{name}»?\nSi no los guardas, se perderán."
        ))
        .set_buttons(buttons)
        .show()
        .await;
    match result {
        MessageDialogResult::Yes => SaveChoice::Save,
        MessageDialogResult::No => SaveChoice::Discard,
        MessageDialogResult::Custom(label) if label == SAVE => SaveChoice::Save,
        MessageDialogResult::Custom(label) if label == DISCARD => SaveChoice::Discard,
        _ => SaveChoice::Cancel,
    }
}
