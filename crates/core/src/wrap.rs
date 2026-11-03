//! Ajuste de línea: reparte cada línea del documento en filas visuales que caben en una
//! anchura dada (en celdas), como el «Ajuste de línea» del Bloc de notas.
//!
//! Las filas son solo de presentación: el texto no cambia y las líneas siguen siendo las
//! del documento (los números de línea, «Ir a línea», la sesión… no se ven afectados).
//! Se corta después de un espacio o tabulador; una palabra más larga que la fila se
//! corta donde no cabe. Los espacios al final de una fila pueden sobresalir (no pasan a
//! la siguiente). Las columnas visuales de cada fila empiezan en cero.

use std::ops::Range;

use crate::{Buffer, Position, display};

/// Fila visual: un tramo `[start, end)` (columnas de carácter) de la línea `line`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub line: usize,
    pub start: usize,
    pub end: usize,
    /// `true` si es la última fila de su línea (después viene un salto de línea real o el
    /// final del documento).
    pub last: bool,
}

impl Row {
    /// `true` si es la primera fila de su línea (la que lleva el número de línea).
    pub fn is_first(&self) -> bool {
        self.start == 0
    }
}

/// Filas de todo el documento para una anchura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapLayout {
    width: usize,
    tab_width: usize,
    rows: Vec<Row>,
    /// Primera fila de cada línea.
    first_row: Vec<usize>,
}

/// Tramos (columnas de carácter) en que se reparte `text` con filas de `width` celdas.
/// Siempre hay al menos un tramo (una línea vacía ocupa una fila).
pub fn wrap_line(text: &str, width: usize, tab_width: usize) -> Vec<Range<usize>> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    let mut rows = Vec::new();
    let mut row_start = 0;
    let mut visual = 0;
    // Columna después del último espacio de la fila: el mejor sitio para cortar.
    let mut last_break = None;
    let mut index = 0;
    while index < chars.len() {
        let c = chars[index];
        let cells = display::char_cells(c, visual, tab_width);
        let blank = c == ' ' || c == '\t';
        if visual + cells > width && index > row_start && !blank {
            let cut = last_break.filter(|&b| b > row_start).unwrap_or(index);
            rows.push(row_start..cut);
            row_start = cut;
            last_break = None;
            // Lo que había entre el corte y aquí pasa a la fila nueva.
            visual = chars[cut..index]
                .iter()
                .fold(0, |v, &c| v + display::char_cells(c, v, tab_width));
            continue;
        }
        visual += cells;
        if blank {
            last_break = Some(index + 1);
        }
        index += 1;
    }
    rows.push(row_start..chars.len());
    rows
}

impl WrapLayout {
    /// Reparte todo `buffer` en filas de `width` celdas.
    pub fn new(buffer: &Buffer, width: usize, tab_width: usize) -> Self {
        let lines = buffer.len_lines();
        let mut rows = Vec::with_capacity(lines);
        let mut first_row = Vec::with_capacity(lines);
        for line in 0..lines {
            first_row.push(rows.len());
            let ranges = wrap_line(&buffer.line_text(line), width, tab_width);
            let count = ranges.len();
            rows.extend(ranges.into_iter().enumerate().map(|(i, range)| Row {
                line,
                start: range.start,
                end: range.end,
                last: i + 1 == count,
            }));
        }
        Self {
            width,
            tab_width,
            rows,
            first_row,
        }
    }

    /// Anchura (en celdas) para la que se calculó.
    pub fn width(&self) -> usize {
        self.width
    }

    pub fn tab_width(&self) -> usize {
        self.tab_width
    }

    /// Número de líneas del documento para el que se calculó.
    pub fn line_count(&self) -> usize {
        self.first_row.len()
    }
}

/// Correspondencia entre filas visuales y posiciones del documento, con o sin ajuste de
/// línea (sin él, cada línea es una fila).
#[derive(Debug, Clone, Copy)]
pub struct Rows<'a> {
    buffer: &'a Buffer,
    wrap: Option<&'a WrapLayout>,
    tab_width: usize,
}

impl<'a> Rows<'a> {
    /// Un `wrap` calculado para otro número de líneas (de una versión anterior del
    /// documento) se ignora.
    pub fn new(buffer: &'a Buffer, wrap: Option<&'a WrapLayout>, tab_width: usize) -> Self {
        let wrap = wrap.filter(|w| w.line_count() == buffer.len_lines());
        Self {
            buffer,
            wrap,
            tab_width,
        }
    }

    pub fn buffer(&self) -> &'a Buffer {
        self.buffer
    }

    /// `true` si hay ajuste de línea.
    pub fn is_wrapped(&self) -> bool {
        self.wrap.is_some()
    }

    /// Número de filas.
    pub fn count(&self) -> usize {
        match self.wrap {
            Some(wrap) => wrap.rows.len(),
            None => self.buffer.len_lines(),
        }
    }

    /// Fila `row` (ajustada a la última si no existe).
    pub fn row(&self, row: usize) -> Row {
        match self.wrap {
            Some(wrap) => {
                let row = row.min(wrap.rows.len().saturating_sub(1));
                wrap.rows.get(row).copied().unwrap_or(Row {
                    line: 0,
                    start: 0,
                    end: 0,
                    last: true,
                })
            }
            None => {
                let line = row.min(self.buffer.len_lines().saturating_sub(1));
                Row {
                    line,
                    start: 0,
                    end: self.buffer.line_len(line),
                    last: true,
                }
            }
        }
    }

    /// Primera fila de una línea.
    pub fn first_row_of(&self, line: usize) -> usize {
        match self.wrap {
            Some(wrap) => wrap
                .first_row
                .get(line)
                .copied()
                .unwrap_or_else(|| wrap.rows.len().saturating_sub(1)),
            None => line,
        }
    }

    /// Fila en la que se ve una posición. Una columna justo en un corte se ve al principio
    /// de la fila siguiente.
    pub fn row_of(&self, position: Position) -> usize {
        let first = self.first_row_of(position.line);
        let Some(wrap) = self.wrap else {
            return first;
        };
        let end = wrap
            .first_row
            .get(position.line + 1)
            .copied()
            .unwrap_or(wrap.rows.len());
        let rows = wrap.rows.get(first..end).unwrap_or_default();
        let inside = rows.partition_point(|row| row.start <= position.column);
        first + inside.saturating_sub(1)
    }

    /// Texto de una fila (sin el salto de línea).
    pub fn text(&self, row: Row) -> String {
        let line = self.buffer.line_text(row.line);
        if self.wrap.is_none() {
            return line;
        }
        line.chars()
            .skip(row.start)
            .take(row.end - row.start)
            .collect()
    }

    /// Fila y celda (columna visual dentro de la fila) de un índice de carácter.
    pub fn locate(&self, index: usize) -> (usize, usize) {
        let position = self.buffer.char_to_position(index);
        let row = self.row_of(position);
        let info = self.row(row);
        let text = self.text(info);
        let column = position.column.saturating_sub(info.start);
        (row, display::visual_column(&text, column, self.tab_width))
    }

    /// Índice de carácter más cercano a la celda `cells` de la fila `row`. En una fila
    /// cortada, pasado el final se queda antes del último carácter (si no, el cursor se
    /// vería al principio de la fila siguiente).
    pub fn index_at(&self, row: usize, cells: f32) -> usize {
        let info = self.row(row);
        let text = self.text(info);
        let mut column = display::char_column_at_visual(&text, cells.max(0.0), self.tab_width);
        let len = info.end - info.start;
        if !info.last && column >= len && len > 0 {
            column = len - 1;
        }
        self.buffer
            .position_to_char(Position::new(info.line, info.start + column))
    }

    /// Destino de mover `index` `delta` filas manteniendo la celda `goal` (la actual si no
    /// se indica). Más allá del principio o del final del documento, va a sus extremos.
    pub fn vertical_target(&self, index: usize, delta: isize, goal: Option<usize>) -> usize {
        let (row, cells) = self.locate(index);
        let goal = goal.unwrap_or(cells);
        match row.checked_add_signed(delta) {
            None => 0,
            Some(target) if target >= self.count() => self.buffer.len_chars(),
            Some(target) => self.index_at(target, goal as f32),
        }
    }

    /// Principio y final de la fila de `index` (para Inicio y Fin con ajuste de línea).
    pub fn row_bounds(&self, index: usize) -> (usize, usize) {
        let (row, _) = self.locate(index);
        let info = self.row(row);
        let start = self
            .buffer
            .position_to_char(Position::new(info.line, info.start));
        let end_column = if info.last || info.end == info.start {
            info.end
        } else {
            info.end - 1
        };
        let end = self
            .buffer
            .position_to_char(Position::new(info.line, end_column));
        (start, end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pieces(text: &str, width: usize) -> Vec<String> {
        let chars: Vec<char> = text.chars().collect();
        wrap_line(text, width, 4)
            .into_iter()
            .map(|r| chars[r].iter().collect())
            .collect()
    }

    #[test]
    fn short_and_empty_lines_are_one_row() {
        assert_eq!(pieces("", 10), vec![""]);
        assert_eq!(pieces("hola", 10), vec!["hola"]);
        assert_eq!(pieces("0123456789", 10), vec!["0123456789"]);
    }

    #[test]
    fn breaks_after_spaces() {
        assert_eq!(
            pieces("el verde que te quiero verde", 10),
            vec!["el verde ", "que te ", "quiero ", "verde"]
        );
    }

    #[test]
    fn trailing_spaces_hang() {
        assert_eq!(pieces("abcd      efgh", 5), vec!["abcd      ", "efgh"]);
    }

    #[test]
    fn long_words_are_cut() {
        assert_eq!(pieces("abcdefghij", 4), vec!["abcd", "efgh", "ij"]);
        assert_eq!(pieces("ab abcdefgh", 4), vec!["ab ", "abcd", "efgh"]);
    }

    #[test]
    fn wide_characters_count_two_cells() {
        assert_eq!(pieces("日本語日本", 4), vec!["日本", "語日", "本"]);
    }

    #[test]
    fn tabs_restart_at_each_row() {
        // Tabulador de 4: "a\tb" ocupa 5 celdas.
        assert_eq!(pieces("a\tb c\td", 6), vec!["a\tb ", "c\td"]);
    }

    fn sample() -> (Buffer, WrapLayout) {
        let buffer = Buffer::from_text("uno dos tres\n\ncuatro");
        let wrap = WrapLayout::new(&buffer, 8, 4);
        (buffer, wrap)
    }

    #[test]
    fn layout_maps_rows_and_lines() {
        let (buffer, wrap) = sample();
        let rows = Rows::new(&buffer, Some(&wrap), 4);
        assert!(rows.is_wrapped());
        // «uno dos » / «tres» / «» / «cuatro»
        assert_eq!(rows.count(), 4);
        assert_eq!(rows.text(rows.row(0)), "uno dos ");
        assert_eq!(rows.text(rows.row(1)), "tres");
        assert!(!rows.row(0).last && rows.row(1).last);
        assert!(rows.row(0).is_first() && !rows.row(1).is_first());
        assert_eq!(rows.first_row_of(2), 3);
        assert_eq!(rows.row(99).line, 2);
    }

    #[test]
    fn positions_at_a_break_belong_to_the_next_row() {
        let (buffer, wrap) = sample();
        let rows = Rows::new(&buffer, Some(&wrap), 4);
        assert_eq!(rows.locate(0), (0, 0));
        assert_eq!(rows.locate(7), (0, 7));
        assert_eq!(rows.locate(8), (1, 0));
        assert_eq!(rows.locate(12), (1, 4));
        assert_eq!(rows.locate(13), (2, 0));
    }

    #[test]
    fn clicking_past_a_cut_row_stays_on_it() {
        let (buffer, wrap) = sample();
        let rows = Rows::new(&buffer, Some(&wrap), 4);
        assert_eq!(rows.index_at(0, 50.0), 7);
        assert_eq!(rows.index_at(1, 50.0), 12);
        assert_eq!(rows.index_at(1, 1.0), 9);
    }

    #[test]
    fn vertical_moves_use_rows() {
        let (buffer, wrap) = sample();
        let rows = Rows::new(&buffer, Some(&wrap), 4);
        // Del principio de «dos» (celda 4) baja a «tres» (celda 4 = final).
        assert_eq!(rows.vertical_target(4, 1, None), 12);
        assert_eq!(rows.vertical_target(12, -1, None), 4);
        assert_eq!(rows.vertical_target(12, -1, Some(1)), 1);
        assert_eq!(rows.vertical_target(1, -1, None), 0);
        assert_eq!(rows.vertical_target(15, 5, None), buffer.len_chars());
    }

    #[test]
    fn row_bounds_stop_before_the_break() {
        let (buffer, wrap) = sample();
        let rows = Rows::new(&buffer, Some(&wrap), 4);
        assert_eq!(rows.row_bounds(2), (0, 7));
        assert_eq!(rows.row_bounds(10), (8, 12));
    }

    #[test]
    fn without_wrap_rows_are_lines() {
        let buffer = Buffer::from_text("a\nbb");
        let rows = Rows::new(&buffer, None, 4);
        assert!(!rows.is_wrapped());
        assert_eq!(rows.count(), 2);
        assert_eq!(
            rows.row(1),
            Row {
                line: 1,
                start: 0,
                end: 2,
                last: true
            }
        );
        assert_eq!(rows.locate(4), (1, 2));
        assert_eq!(rows.vertical_target(1, 1, None), 3);
    }

    #[test]
    fn stale_layout_is_ignored() {
        let (_, wrap) = sample();
        let other = Buffer::from_text("una sola línea");
        assert!(!Rows::new(&other, Some(&wrap), 4).is_wrapped());
    }
}
