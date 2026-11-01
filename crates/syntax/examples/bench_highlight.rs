//! Mide el tiempo de resaltado de un archivo: `cargo run --release -p lorca-syntax --example bench_highlight -- archivo`.

use std::time::Instant;

use lorca_syntax::{Language, highlight};

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("uso: bench_highlight ARCHIVO");
        return;
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("no se pudo leer {path}");
        return;
    };
    let language = Language::from_path(std::path::Path::new(&path));
    // La primera vez incluye compilar las consultas.
    for round in 0..3 {
        let start = Instant::now();
        let result = highlight(language, &text);
        let lines = result.ok().flatten().map_or(0, |h| h.len_lines());
        println!("ronda {round}: {lines} líneas en {:?}", start.elapsed());
    }
}
