//! Mide el cálculo del ajuste de línea en un texto grande.
//!
//! `cargo run --release -p lorca-core --example bench_wrap`

use std::time::Instant;

use lorca_core::{Buffer, WrapLayout};

fn main() {
    let paragraph = "Verde que te quiero verde. Verde viento. Verdes ramas. ".repeat(20);
    let text = format!("{paragraph}\n\n").repeat(1_000);
    let buffer = Buffer::from_text(&text);
    let start = Instant::now();
    let layout = WrapLayout::new(&buffer, 100, 4);
    println!(
        "{} MB, {} líneas → ancho {}: {:?}",
        text.len() / 1_000_000,
        buffer.len_lines(),
        layout.width(),
        start.elapsed()
    );
}
