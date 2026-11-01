//! Buffer de texto basado en [`ropey::Rope`].

use std::fmt;
use std::ops::Range;

use ropey::{Rope, RopeSlice};

use crate::{CoreError, LineEnding, Position};

/// Texto de un documento. Internamente solo contiene saltos de línea LF.
///
/// Todos los índices son de carácter Unicode. Los métodos de consulta ajustan los
/// índices fuera de rango; los de modificación devuelven [`CoreError`].
#[derive(Debug, Clone, Default)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Crea un buffer normalizando los finales de línea a LF.
    pub fn from_text(text: &str) -> Self {
        Self {
            rope: Rope::from_str(&LineEnding::normalize(text)),
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    /// Número de líneas. Un buffer vacío tiene una línea y un salto final abre otra vacía.
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Línea `line` incluyendo su salto final, si existe.
    pub fn line(&self, line: usize) -> Option<RopeSlice<'_>> {
        self.rope.get_line(line)
    }

    /// Longitud en caracteres de la línea, sin contar el salto. Cero si no existe.
    pub fn line_len(&self, line: usize) -> usize {
        self.line(line).map_or(0, |slice| {
            let len = slice.len_chars();
            if len > 0 && slice.char(len - 1) == '\n' {
                len - 1
            } else {
                len
            }
        })
    }

    /// Texto de la línea sin el salto final.
    pub fn line_text(&self, line: usize) -> String {
        self.line(line)
            .map(|slice| {
                let mut text = slice.to_string();
                if text.ends_with('\n') {
                    text.pop();
                }
                text
            })
            .unwrap_or_default()
    }

    /// Índice de carácter donde empieza la línea (ajustado a la última línea).
    pub fn line_start(&self, line: usize) -> usize {
        let line = line.min(self.len_lines().saturating_sub(1));
        self.rope.line_to_char(line)
    }

    /// Índice de carácter donde termina la línea (antes del salto).
    pub fn line_end(&self, line: usize) -> usize {
        let line = line.min(self.len_lines().saturating_sub(1));
        self.line_start(line) + self.line_len(line)
    }

    pub fn char_at(&self, index: usize) -> Option<char> {
        self.rope.get_char(index)
    }

    /// Línea que contiene el índice (ajustado al final del buffer).
    pub fn char_to_line(&self, index: usize) -> usize {
        self.rope.char_to_line(index.min(self.len_chars()))
    }

    /// Convierte un índice de carácter en línea y columna.
    pub fn char_to_position(&self, index: usize) -> Position {
        let index = index.min(self.len_chars());
        let line = self.rope.char_to_line(index);
        Position::new(line, index - self.rope.line_to_char(line))
    }

    /// Convierte línea y columna en índice, ajustando ambas a valores válidos.
    pub fn position_to_char(&self, position: Position) -> usize {
        let line = position.line.min(self.len_lines().saturating_sub(1));
        self.line_start(line) + position.column.min(self.line_len(line))
    }

    /// Copia el texto de un rango.
    pub fn slice(&self, range: Range<usize>) -> Result<String, CoreError> {
        self.check_range(&range)?;
        Ok(self.rope.slice(range).to_string())
    }

    /// Inserta texto tal cual en `at`. Quien llama es responsable de normalizarlo a LF.
    pub fn insert(&mut self, at: usize, text: &str) -> Result<(), CoreError> {
        self.rope
            .try_insert(at, text)
            .map_err(|_| CoreError::OutOfBounds {
                index: at,
                len: self.rope.len_chars(),
            })
    }

    /// Elimina el rango indicado.
    pub fn remove(&mut self, range: Range<usize>) -> Result<(), CoreError> {
        self.check_range(&range)?;
        self.rope.remove(range);
        Ok(())
    }

    fn check_range(&self, range: &Range<usize>) -> Result<(), CoreError> {
        let len = self.len_chars();
        if range.start > range.end || range.end > len {
            return Err(CoreError::InvalidRange {
                start: range.start,
                end: range.end,
                len,
            });
        }
        Ok(())
    }
}

impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for chunk in self.rope.chunks() {
            f.write_str(chunk)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_has_one_line() {
        let b = Buffer::new();
        assert!(b.is_empty());
        assert_eq!(b.len_lines(), 1);
        assert_eq!(b.line_len(0), 0);
    }

    #[test]
    fn from_text_normalizes_line_endings() {
        let b = Buffer::from_text("a\r\nb\rc");
        assert_eq!(b.to_string(), "a\nb\nc");
        assert_eq!(b.len_lines(), 3);
    }

    #[test]
    fn line_queries() {
        let b = Buffer::from_text("hola\nñandú\n");
        assert_eq!(b.len_lines(), 3);
        assert_eq!(b.line_len(0), 4);
        assert_eq!(b.line_len(1), 5);
        assert_eq!(b.line_len(2), 0);
        assert_eq!(b.line_len(99), 0);
        assert_eq!(b.line_text(1), "ñandú");
        assert_eq!(b.line_start(1), 5);
        assert_eq!(b.line_end(1), 10);
    }

    #[test]
    fn position_conversions() {
        let b = Buffer::from_text("ab\ncde");
        assert_eq!(b.char_to_position(4), Position::new(1, 1));
        assert_eq!(b.position_to_char(Position::new(1, 1)), 4);
        // Columna y línea fuera de rango se ajustan.
        assert_eq!(b.position_to_char(Position::new(0, 50)), 2);
        assert_eq!(b.position_to_char(Position::new(9, 0)), 3);
        assert_eq!(b.char_to_position(999), Position::new(1, 3));
    }

    #[test]
    fn insert_and_remove() {
        let mut b = Buffer::from_text("verde");
        b.insert(5, " viento").unwrap();
        assert_eq!(b.to_string(), "verde viento");
        b.remove(0..6).unwrap();
        assert_eq!(b.to_string(), "viento");
    }

    #[test]
    fn invalid_operations_return_errors() {
        let mut b = Buffer::from_text("abc");
        assert!(matches!(
            b.insert(10, "x"),
            Err(CoreError::OutOfBounds { .. })
        ));
        assert!(matches!(
            b.remove(2..10),
            Err(CoreError::InvalidRange { .. })
        ));
        #[allow(clippy::reversed_empty_ranges)]
        let reversed = 2..1;
        assert!(matches!(
            b.slice(reversed),
            Err(CoreError::InvalidRange { .. })
        ));
        assert_eq!(b.to_string(), "abc");
    }
}
