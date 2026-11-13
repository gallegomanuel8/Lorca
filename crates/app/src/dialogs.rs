//! Diálogos de archivos del sistema (rfd), siempre asíncronos para no bloquear la
//! interfaz. Las confirmaciones son propias (`prompt.rs`).

use std::path::PathBuf;

use rfd::AsyncFileDialog;

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

/// Pide una o varias imágenes para insertar.
pub async fn pick_images(directory: Option<PathBuf>) -> Vec<PathBuf> {
    let mut dialog = AsyncFileDialog::new()
        .set_title("Insertar imagen")
        .add_filter("Imágenes", &lorca_workspace::IMAGE_EXTENSIONS);
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    dialog
        .pick_files()
        .await
        .map(|files| files.iter().map(|f| f.path().to_path_buf()).collect())
        .unwrap_or_default()
}

/// Pide una carpeta para buscar en ella.
pub async fn pick_folder(directory: Option<PathBuf>) -> Option<PathBuf> {
    let mut dialog = AsyncFileDialog::new().set_title("Buscar en carpeta");
    if let Some(directory) = directory {
        dialog = dialog.set_directory(directory);
    }
    dialog.pick_folder().await.map(|f| f.path().to_path_buf())
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
