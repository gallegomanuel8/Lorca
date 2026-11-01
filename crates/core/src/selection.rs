//! Selección simple: un ancla y una cabeza expresadas como índices de carácter.

use std::ops::Range;

/// Selección de texto. La cabeza es donde está el cursor; el ancla, donde empezó la selección.
///
/// Una selección vacía (`anchor == head`) representa un cursor sin texto seleccionado.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Selection {
    pub anchor: usize,
    pub head: usize,
}

impl Selection {
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// Crea un cursor (selección vacía) en `at`.
    pub const fn cursor(at: usize) -> Self {
        Self {
            anchor: at,
            head: at,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Índice inicial (el menor de ancla y cabeza).
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Índice final (el mayor de ancla y cabeza).
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Número de caracteres seleccionados.
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    pub fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    /// Ajusta ambos extremos para que no superen `len`.
    pub fn clamped(self, len: usize) -> Self {
        Self::new(self.anchor.min(len), self.head.min(len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_is_empty() {
        let s = Selection::cursor(3);
        assert!(s.is_empty());
        assert_eq!(s.range(), 3..3);
    }

    #[test]
    fn range_is_ordered_regardless_of_direction() {
        assert_eq!(Selection::new(5, 2).range(), 2..5);
        assert_eq!(Selection::new(2, 5).range(), 2..5);
    }

    #[test]
    fn clamped_limits_both_ends() {
        assert_eq!(Selection::new(10, 20).clamped(8), Selection::new(8, 8));
    }
}
