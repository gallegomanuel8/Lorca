//! Columnas visuales: conversión entre columnas de carácter y celdas en pantalla.
//!
//! Supone una rejilla monoespaciada: cada carácter ocupa 0, 1 o 2 celdas según
//! `unicode-width`, y un tabulador avanza hasta la siguiente parada de tabulación.

use unicode_width::UnicodeWidthChar;

/// Celdas que ocupa `c` si empieza en la columna visual `visual_column`.
pub fn char_cells(c: char, visual_column: usize, tab_width: usize) -> usize {
    if c == '\t' {
        let tab_width = tab_width.max(1);
        tab_width - visual_column % tab_width
    } else {
        // Los caracteres de control no tienen anchura definida; se dibujan en una celda.
        c.width().unwrap_or(1)
    }
}

/// Columna visual (en celdas) de la columna de carácter `char_column` de `line`.
pub fn visual_column(line: &str, char_column: usize, tab_width: usize) -> usize {
    line.chars()
        .take(char_column)
        .fold(0, |visual, c| visual + char_cells(c, visual, tab_width))
}

/// Columna de carácter más cercana a la posición visual `cells` (puede ser fraccionaria).
///
/// Si la posición cae en la mitad derecha de un carácter, devuelve la columna siguiente.
pub fn char_column_at_visual(line: &str, cells: f32, tab_width: usize) -> usize {
    let mut visual = 0usize;
    for (index, c) in line.chars().enumerate() {
        let width = char_cells(c, visual, tab_width);
        if cells < visual as f32 + width as f32 / 2.0 {
            return index;
        }
        visual += width;
    }
    line.chars().count()
}

/// Fragmento de `line` visible a partir de la celda `first_cell`, con los tabuladores
/// expandidos a espacios. Devuelve el texto y la celda en la que empieza.
///
/// Se limita a `max_cells` celdas para no maquetar líneas larguísimas fuera de la vista.
pub fn visible_text(
    line: &str,
    first_cell: usize,
    max_cells: usize,
    tab_width: usize,
) -> (String, usize) {
    let mut out = String::new();
    let mut visual = 0usize;
    let mut start = None;
    for c in line.chars() {
        let width = char_cells(c, visual, tab_width);
        if start.is_none() && visual + width > first_cell {
            start = Some(visual);
        }
        if let Some(start) = start {
            if visual >= start + max_cells {
                break;
            }
            if c == '\t' {
                out.extend(std::iter::repeat_n(' ', width));
            } else {
                out.push(c);
            }
        }
        visual += width;
    }
    (out, start.unwrap_or(visual))
}

/// Tramo de texto visible con un mismo estilo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run<T> {
    /// Texto con los tabuladores ya expandidos a espacios.
    pub text: String,
    /// Celda en la que empieza.
    pub start_cell: usize,
    pub style: Option<T>,
}

/// Como [`visible_text`], pero dividido en tramos según el estilo de cada carácter.
///
/// `style_at` recibe el índice de carácter dentro de la línea, en orden creciente.
pub fn visible_runs<T: Copy + PartialEq>(
    line: &str,
    first_cell: usize,
    max_cells: usize,
    tab_width: usize,
    mut style_at: impl FnMut(usize) -> Option<T>,
) -> Vec<Run<T>> {
    let mut runs: Vec<Run<T>> = Vec::new();
    let mut visual = 0usize;
    let mut start: Option<usize> = None;
    for (index, c) in line.chars().enumerate() {
        let width = char_cells(c, visual, tab_width);
        if start.is_none() && visual + width > first_cell {
            start = Some(visual);
        }
        if let Some(start) = start {
            if visual >= start + max_cells {
                break;
            }
            let style = style_at(index);
            let run = match runs.last_mut() {
                Some(run) if run.style == style => run,
                _ => {
                    runs.push(Run {
                        text: String::new(),
                        start_cell: visual,
                        style,
                    });
                    match runs.last_mut() {
                        Some(run) => run,
                        None => break,
                    }
                }
            };
            if c == '\t' {
                run.text.extend(std::iter::repeat_n(' ', width));
            } else {
                run.text.push(c);
            }
        }
        visual += width;
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_advance_to_next_stop() {
        assert_eq!(visual_column("\tx", 1, 4), 4);
        assert_eq!(visual_column("ab\tx", 3, 4), 4);
        assert_eq!(visual_column("abcd\tx", 5, 4), 8);
    }

    #[test]
    fn wide_characters_take_two_cells() {
        assert_eq!(visual_column("日本x", 2, 4), 4);
        assert_eq!(visual_column("ñandú", 5, 4), 5);
    }

    #[test]
    fn hit_testing_rounds_to_nearest_boundary() {
        assert_eq!(char_column_at_visual("abc", 0.4, 4), 0);
        assert_eq!(char_column_at_visual("abc", 0.6, 4), 1);
        assert_eq!(char_column_at_visual("abc", 99.0, 4), 3);
        assert_eq!(char_column_at_visual("\tx", 1.0, 4), 0);
        assert_eq!(char_column_at_visual("\tx", 3.0, 4), 1);
        assert_eq!(char_column_at_visual("日本", 2.6, 4), 1);
    }

    #[test]
    fn runs_split_by_style() {
        let style = |i: usize| (i < 2).then_some('k');
        let runs = visible_runs("fn main", 0, 80, 4, style);
        assert_eq!(
            runs,
            vec![
                Run {
                    text: "fn".into(),
                    start_cell: 0,
                    style: Some('k')
                },
                Run {
                    text: " main".into(),
                    start_cell: 2,
                    style: None
                },
            ]
        );
    }

    #[test]
    fn runs_respect_scroll_and_tabs() {
        let runs = visible_runs("\tab", 2, 80, 4, |i| (i == 0).then_some(1));
        assert_eq!(
            runs,
            vec![
                Run {
                    text: "    ".into(),
                    start_cell: 0,
                    style: Some(1)
                },
                Run {
                    text: "ab".into(),
                    start_cell: 4,
                    style: None
                },
            ]
        );
        let runs = visible_runs("abcdef", 3, 2, 4, |_| None::<u8>);
        assert_eq!(
            runs,
            vec![Run {
                text: "de".into(),
                start_cell: 3,
                style: None
            }]
        );
    }

    #[test]
    fn visible_text_expands_tabs_and_skips_hidden_part() {
        assert_eq!(visible_text("\tfn", 0, 80, 4), ("    fn".to_owned(), 0));
        assert_eq!(visible_text("abcdef", 2, 3, 4), ("cde".to_owned(), 2));
        // Un carácter ancho cortado por el borde se incluye entero.
        assert_eq!(visible_text("日本", 1, 10, 4), ("日本".to_owned(), 0));
        assert_eq!(visible_text("ab", 10, 10, 4), (String::new(), 2));
    }
}
