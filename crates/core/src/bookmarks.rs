//! Marcadores de línea que se desplazan con las ediciones.

use std::collections::BTreeSet;

use crate::{Buffer, Edit};

/// Conjunto de líneas marcadas (índices desde cero).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bookmarks {
    lines: BTreeSet<usize>,
}

impl Bookmarks {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = usize> + '_ {
        self.lines.iter().copied()
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn contains(&self, line: usize) -> bool {
        self.lines.contains(&line)
    }

    /// Marca o desmarca la línea. Devuelve `true` si queda marcada.
    pub fn toggle(&mut self, line: usize) -> bool {
        if self.lines.remove(&line) {
            false
        } else {
            self.lines.insert(line);
            true
        }
    }

    pub fn insert(&mut self, line: usize) {
        self.lines.insert(line);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Primera línea marcada después de `line`; con `wrap`, vuelve a la primera.
    pub fn next_after(&self, line: usize, wrap: bool) -> Option<usize> {
        self.lines
            .range(line + 1..)
            .next()
            .or_else(|| if wrap { self.lines.iter().next() } else { None })
            .copied()
    }

    /// Última línea marcada antes de `line`; con `wrap`, vuelve a la última.
    pub fn previous_before(&self, line: usize, wrap: bool) -> Option<usize> {
        self.lines
            .range(..line)
            .next_back()
            .or_else(|| {
                if wrap {
                    self.lines.iter().next_back()
                } else {
                    None
                }
            })
            .copied()
    }

    /// Ajusta las líneas marcadas para una edición que **aún no** se ha aplicado.
    ///
    /// Insertar saltos de línea desplaza hacia abajo los marcadores siguientes (y el de la
    /// línea si se inserta al principio de ella). Borrar líneas lleva sus marcadores a la
    /// línea donde empieza el borrado.
    pub(crate) fn adjust(&mut self, buffer: &Buffer, edit: &Edit) {
        if self.lines.is_empty() {
            return;
        }
        match edit {
            Edit::Insert { at, text } => {
                let added = text.matches('\n').count();
                if added == 0 {
                    return;
                }
                let line = buffer.char_to_line(*at);
                let first_moved = if *at == buffer.line_start(line) {
                    line
                } else {
                    line + 1
                };
                self.lines = self
                    .lines
                    .iter()
                    .map(|&l| if l >= first_moved { l + added } else { l })
                    .collect();
            }
            Edit::Delete { at, text } => {
                let removed = text.matches('\n').count();
                if removed == 0 {
                    return;
                }
                let start = buffer.char_to_line(*at);
                let end = start + removed;
                self.lines = self
                    .lines
                    .iter()
                    .map(|&l| match l {
                        l if l > end => l - removed,
                        l if l > start => start,
                        l => l,
                    })
                    .collect();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(lines: &[usize]) -> Bookmarks {
        let mut b = Bookmarks::default();
        lines.iter().for_each(|&l| b.insert(l));
        b
    }

    #[test]
    fn toggle_and_navigation() {
        let mut b = Bookmarks::default();
        assert!(b.toggle(3));
        assert!(b.toggle(7));
        assert!(!b.toggle(3));
        b.insert(1);
        assert_eq!(b.next_after(1, false), Some(7));
        assert_eq!(b.next_after(7, false), None);
        assert_eq!(b.next_after(7, true), Some(1));
        assert_eq!(b.previous_before(7, false), Some(1));
        assert_eq!(b.previous_before(1, true), Some(7));
    }

    #[test]
    fn insert_newlines_shifts_following_lines() {
        let buffer = Buffer::from_text("a\nb\nc");
        let mut b = marks(&[0, 1, 2]);
        // Insertar dentro de la línea 0: solo bajan las siguientes.
        b.adjust(
            &buffer,
            &Edit::Insert {
                at: 1,
                text: "\n\n".into(),
            },
        );
        assert_eq!(b.iter().collect::<Vec<_>>(), [0, 3, 4]);
    }

    #[test]
    fn insert_at_line_start_moves_that_line() {
        let buffer = Buffer::from_text("a\nb");
        let mut b = marks(&[1]);
        b.adjust(
            &buffer,
            &Edit::Insert {
                at: 2,
                text: "nueva\n".into(),
            },
        );
        assert_eq!(b.iter().collect::<Vec<_>>(), [2]);
    }

    #[test]
    fn deleting_lines_merges_and_shifts() {
        let buffer = Buffer::from_text("a\nb\nc\nd");
        let mut b = marks(&[1, 2, 3]);
        // Borrar "b\nc\n" (líneas 1 y 2 enteras).
        b.adjust(
            &buffer,
            &Edit::Delete {
                at: 2,
                text: "b\nc\n".into(),
            },
        );
        assert_eq!(b.iter().collect::<Vec<_>>(), [1]);
    }

    #[test]
    fn edits_without_newlines_change_nothing() {
        let buffer = Buffer::from_text("a\nb");
        let mut b = marks(&[1]);
        b.adjust(
            &buffer,
            &Edit::Insert {
                at: 0,
                text: "xyz".into(),
            },
        );
        b.adjust(
            &buffer,
            &Edit::Delete {
                at: 0,
                text: "a".into(),
            },
        );
        assert_eq!(b.iter().collect::<Vec<_>>(), [1]);
    }
}
