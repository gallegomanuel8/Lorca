//! Conversión de Markdown a HTML con pulldown-cmark.

use pulldown_cmark::{Options, Parser, html};

/// Convierte Markdown (CommonMark con tablas, tachado, listas de tareas y notas al pie) a
/// un fragmento HTML.
pub fn markdown_to_html(text: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut out = String::with_capacity(text.len() * 3 / 2);
    html::push_html(&mut out, Parser::new_ext(text, options));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_markdown() {
        let html = markdown_to_html("# Romance\n\n*verde* y **viento**\n");
        assert!(html.contains("<h1>Romance</h1>"));
        assert!(html.contains("<em>verde</em>"));
        assert!(html.contains("<strong>viento</strong>"));
    }

    #[test]
    fn extensions() {
        let html = markdown_to_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n~~no~~\n\n- [x] hecho\n");
        assert!(html.contains("<table>"));
        assert!(html.contains("<del>no</del>"));
        assert!(html.contains("checkbox"));
    }
}
