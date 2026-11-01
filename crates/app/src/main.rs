//! Binario `lorca`: interfaz gráfica del editor.
//!
//! Uso: `lorca [ARCHIVO...]` abre los archivos indicados en pestañas.

mod app;
mod dialogs;
mod editor;
mod find;
mod key_filter;
mod shortcuts;
mod welcome;

use app::Lorca;

fn main() -> anyhow::Result<()> {
    iced::application(Lorca::boot, Lorca::update, Lorca::view)
        .title(Lorca::title)
        .subscription(Lorca::subscription)
        .theme(Lorca::theme)
        .exit_on_close_request(false)
        .window_size((1100.0, 720.0))
        .centered()
        .run()?;
    Ok(())
}
