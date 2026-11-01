//! Filas de resultados («línea: texto con la coincidencia») a partir de coincidencias.

use std::ops::Range;

/// Longitud máxima de la vista previa de cada resultado.
const PREVIEW_CHARS: usize = 160;

/// Una coincidencia lista para mostrar en una lista de resultados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// Línea (desde cero).
    pub line: usize,
    /// Rango en el texto completo (caracteres).
    pub range: Range<usize>,
    /// Texto de la línea (recortado, con tabuladores como espacios).
    pub preview: String,
    /// Parte de `preview` que coincide (caracteres).
    pub highlight: Range<usize>,
}

/// Prepara las filas de resultados de `text` para las coincidencias dadas (ordenadas).
pub fn build_hits(text: &str, matches: &[Range<usize>]) -> Vec<Hit> {
    // Inicio (en caracteres) de cada línea.
    let mut line_starts = vec![0usize];
    for (index, c) in text.chars().enumerate() {
        if c == '\n' {
            line_starts.push(index + 1);
        }
    }
    let lines: Vec<&str> = text.split('\n').collect();

    matches
        .iter()
        .map(|range| {
            let line = line_starts
                .partition_point(|&start| start <= range.start)
                .saturating_sub(1);
            let column = range.start - line_starts.get(line).copied().unwrap_or(0);
            let chars: Vec<char> = lines
                .get(line)
                .copied()
                .unwrap_or_default()
                .chars()
                .collect();
            let match_len = range.len().min(chars.len().saturating_sub(column));
            // Ventana de texto alrededor de la coincidencia.
            let first = column.saturating_sub(PREVIEW_CHARS / 3);
            let last = (column + match_len + PREVIEW_CHARS).min(chars.len());
            let preview: String = chars.get(first..last).unwrap_or_default().iter().collect();
            let start = column - first;
            Hit {
                line,
                range: range.clone(),
                preview: preview.replace('\t', " "),
                highlight: start..start + match_len,
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)] // Las listas de un solo rango son intencionadas.
mod tests {
    use super::*;

    fn highlighted(hit: &Hit) -> String {
        hit.preview
            .chars()
            .skip(hit.highlight.start)
            .take(hit.highlight.len())
            .collect()
    }

    #[test]
    fn hits_have_line_and_highlight() {
        let hits = build_hits("uno\n\tdos verde tres", &[9..14]);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[0].preview, " dos verde tres");
        assert_eq!(highlighted(&hits[0]), "verde");
    }

    #[test]
    fn long_lines_are_trimmed_around_the_match() {
        let line = format!("{}aguja{}", "x".repeat(1000), "y".repeat(1000));
        let hits = build_hits(&line, &[1000..1005]);
        assert!(hits[0].preview.chars().count() <= PREVIEW_CHARS + PREVIEW_CHARS / 3 + 5);
        assert_eq!(highlighted(&hits[0]), "aguja");
    }

    #[test]
    fn multiline_match_highlight_stops_at_line_end() {
        let hits = build_hits("ab\ncd", &[1..4]);
        assert_eq!(hits[0].highlight, 1..2);
    }

    #[test]
    fn several_matches_on_several_lines() {
        let hits = build_hits("a\nb a\na", &[0..1, 4..5, 6..7]);
        let lines: Vec<_> = hits.iter().map(|h| h.line).collect();
        assert_eq!(lines, [0, 1, 2]);
    }
}
