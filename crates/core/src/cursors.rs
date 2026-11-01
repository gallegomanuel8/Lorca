//! Varias selecciones a la vez (multicursor), ordenadas y sin solapes.

use crate::Selection;

/// Conjunto de selecciones con una principal.
///
/// Invariantes: hay al menos una selección; están ordenadas por inicio; no se solapan
/// (las que se solapan o se tocan con un cursor se fusionan); `primary` es un índice válido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursors {
    selections: Vec<Selection>,
    primary: usize,
}

impl Default for Cursors {
    fn default() -> Self {
        Self::single(Selection::default())
    }
}

impl From<Selection> for Cursors {
    fn from(selection: Selection) -> Self {
        Self::single(selection)
    }
}

impl Cursors {
    pub fn single(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
            primary: 0,
        }
    }

    /// Crea el conjunto normalizando: ordena y fusiona. `primary` indica cuál de
    /// `selections` es la principal (si queda fusionada, la principal es la fusión).
    pub fn new(selections: Vec<Selection>, primary: usize) -> Self {
        if selections.is_empty() {
            return Self::default();
        }
        let primary = primary.min(selections.len() - 1);
        let mut items: Vec<(Selection, bool)> = selections
            .into_iter()
            .enumerate()
            .map(|(i, s)| (s, i == primary))
            .collect();
        items.sort_by_key(|(s, _)| (s.start(), s.end()));

        let mut merged: Vec<(Selection, bool)> = Vec::with_capacity(items.len());
        for (selection, is_primary) in items {
            match merged.last_mut() {
                Some((last, last_primary))
                    if selection.start() < last.end()
                        || (selection.start() == last.end()
                            && (selection.is_empty() || last.is_empty())) =>
                {
                    let start = last.start().min(selection.start());
                    let end = last.end().max(selection.end());
                    // La orientación la decide la principal, si está entre las fusionadas.
                    let backwards = if is_primary {
                        selection.head < selection.anchor
                    } else {
                        last.head < last.anchor
                    };
                    *last = if backwards {
                        Selection::new(end, start)
                    } else {
                        Selection::new(start, end)
                    };
                    *last_primary |= is_primary;
                }
                _ => merged.push((selection, is_primary)),
            }
        }
        let primary = merged.iter().position(|(_, p)| *p).unwrap_or(0);
        Self {
            selections: merged.into_iter().map(|(s, _)| s).collect(),
            primary,
        }
    }

    pub fn primary(&self) -> Selection {
        self.selections
            .get(self.primary)
            .copied()
            .unwrap_or_default()
    }

    pub fn primary_index(&self) -> usize {
        self.primary
    }

    pub fn as_slice(&self) -> &[Selection] {
        &self.selections
    }

    pub fn len(&self) -> usize {
        self.selections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }

    /// Ajusta todas las selecciones a la longitud del texto (y vuelve a normalizar).
    pub fn clamped(self, len: usize) -> Self {
        let primary = self.primary;
        Self::new(
            self.selections
                .into_iter()
                .map(|s| s.clamped(len))
                .collect(),
            primary,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_and_keeps_primary() {
        let c = Cursors::new(
            vec![
                Selection::cursor(9),
                Selection::cursor(2),
                Selection::cursor(5),
            ],
            0,
        );
        assert_eq!(
            c.as_slice(),
            &[
                Selection::cursor(2),
                Selection::cursor(5),
                Selection::cursor(9)
            ]
        );
        assert_eq!(c.primary(), Selection::cursor(9));
    }

    #[test]
    fn duplicate_cursors_merge() {
        let c = Cursors::new(vec![Selection::cursor(3), Selection::cursor(3)], 1);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn overlapping_selections_merge_into_union() {
        let c = Cursors::new(
            vec![
                Selection::new(0, 5),
                Selection::new(3, 8),
                Selection::cursor(20),
            ],
            1,
        );
        assert_eq!(c.as_slice(), &[Selection::new(0, 8), Selection::cursor(20)]);
        assert_eq!(c.primary(), Selection::new(0, 8));
    }

    #[test]
    fn cursor_touching_a_selection_merges_but_adjacent_selections_do_not() {
        let c = Cursors::new(vec![Selection::new(0, 5), Selection::cursor(5)], 0);
        assert_eq!(c.len(), 1);
        let c = Cursors::new(vec![Selection::new(0, 5), Selection::new(5, 8)], 0);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn empty_list_becomes_default() {
        assert_eq!(Cursors::new(Vec::new(), 3), Cursors::default());
    }
}
