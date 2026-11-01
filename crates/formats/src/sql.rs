//! Formateo de SQL con `sqlformat`.

use sqlformat::{FormatOptions, Indent, QueryParams};

/// Formatea SQL con sangría de cuatro espacios y palabras clave en mayúsculas.
pub fn format_sql(text: &str) -> String {
    let options = FormatOptions {
        indent: Indent::Spaces(4),
        uppercase: Some(true),
        lines_between_queries: 2,
        ..FormatOptions::default()
    };
    let mut formatted = sqlformat::format(text, &QueryParams::None, &options);
    if text.ends_with('\n') && !formatted.ends_with('\n') {
        formatted.push('\n');
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_uppercases() {
        let formatted =
            format_sql("select titulo, anio from obras where autor = 'Lorca' order by anio");
        assert!(formatted.starts_with("SELECT"));
        assert!(formatted.contains("\nFROM\n    obras"));
        assert!(formatted.contains("'Lorca'"));
    }

    #[test]
    fn keeps_trailing_newline() {
        assert!(format_sql("select 1;\n").ends_with('\n'));
    }
}
