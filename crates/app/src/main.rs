//! Binario `lorca`: interfaz gráfica del editor.
//!
//! Uso: `lorca [ARCHIVO...]` abre los archivos indicados en pestañas.

// En Windows, sin esto el ejecutable es de consola y abre una ventana de cmd detrás de la
// del editor. En depuración se mantiene la consola para ver los mensajes.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod context_menu;
mod dialogs;
mod editor;
mod find;
mod format_bar;
mod instance;
mod key_filter;
mod lsp_view;
#[cfg(target_os = "macos")]
mod macos;
mod menu;
mod palette;
mod prompt;
mod shortcut_labels;
mod shortcuts;
mod theme;
mod tour;
mod update_dialog;
mod updates;
mod watcher;
mod welcome;

use app::Lorca;

/// Icono de la ventana (barra de tareas en Windows y Linux; macOS usa el del bundle).
const ICON: &[u8] = include_bytes!("../../../assets/icons/png/lorca-256.png");

fn main() -> anyhow::Result<()> {
    // Si Lorca ya está abierto, le pasa los archivos y termina sin abrir otra ventana.
    let paths: Vec<std::path::PathBuf> = std::env::args_os().skip(1).map(Into::into).collect();
    if !instance::start(&paths) {
        return Ok(());
    }
    // macOS entrega «Abrir con» y el doble clic como un evento, no como argumentos.
    #[cfg(target_os = "macos")]
    macos::install_open_documents();
    let window = iced::window::Settings {
        size: app::WINDOW_SIZE,
        position: iced::window::Position::Centered,
        exit_on_close_request: false,
        // Si no se pudiera decodificar, la ventana sale sin icono.
        icon: iced::window::icon::from_file_data(ICON, None).ok(),
        // En Linux el escritorio asocia la ventana a su `.desktop` (y a su icono) por el
        // app_id de Wayland / WM_CLASS de X11; vacío, sale el icono genérico.
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "lorca".to_owned(),
            ..Default::default()
        },
        ..iced::window::Settings::default()
    };
    iced::application(Lorca::boot, Lorca::update, Lorca::view)
        .title(Lorca::title)
        .subscription(Lorca::subscription)
        .theme(Lorca::theme)
        .window(window)
        .run()?;
    Ok(())
}
