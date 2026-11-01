//! Binario `lorca`: interfaz gráfica del editor.
//!
//! Uso: `lorca [ARCHIVO...]` abre los archivos indicados en pestañas.

mod app;
mod context_menu;
mod dialogs;
mod editor;
mod find;
mod key_filter;
mod shortcut_labels;
mod shortcuts;
mod welcome;

use app::Lorca;

fn main() -> anyhow::Result<()> {
    iced::application(Lorca::boot, Lorca::update, Lorca::view)
        .title(Lorca::title)
        .subscription(Lorca::subscription)
        .theme(Lorca::theme)
        .exit_on_close_request(false)
        .window_size(app::WINDOW_SIZE)
        .centered()
        .run()?;
    Ok(())
}
