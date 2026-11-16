//! Convierte un Markdown a PDF: `cargo run -p lorca-formats --example md2pdf -- entrada.md salida.pdf`.

use lorca_formats::{PdfFonts, markdown_to_pdf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
        eprintln!("uso: md2pdf ENTRADA.md SALIDA.pdf");
        return;
    };
    let result = std::fs::read_to_string(input)
        .map_err(|e| e.to_string())
        .and_then(|text| {
            let fonts = PdfFonts::system().map_err(|e| e.to_string())?;
            // Imágenes relativas a la carpeta del Markdown.
            let base = std::path::Path::new(input)
                .parent()
                .map(std::path::Path::to_path_buf)
                .unwrap_or_default();
            let resolve = |target: &str| Some(base.join(target));
            markdown_to_pdf(&text, input, &fonts, &resolve).map_err(|e| e.to_string())
        })
        .and_then(|pdf| std::fs::write(output, pdf).map_err(|e| e.to_string()));
    if let Err(error) = result {
        eprintln!("{error}");
    }
}
