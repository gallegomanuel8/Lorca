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
            markdown_to_pdf(&text, input, &fonts).map_err(|e| e.to_string())
        })
        .and_then(|pdf| std::fs::write(output, pdf).map_err(|e| e.to_string()));
    if let Err(error) = result {
        eprintln!("{error}");
    }
}
