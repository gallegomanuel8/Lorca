//! Navegación entre coincidencias a partir de la selección actual.

use std::ops::Range;

/// Índice de la coincidencia que es exactamente la selección, si la hay.
pub fn match_index(matches: &[Range<usize>], selection: &Range<usize>) -> Option<usize> {
    let index = matches.partition_point(|m| m.start < selection.start);
    matches
        .get(index)
        .filter(|m| *m == selection)
        .map(|_| index)
}

/// Siguiente coincidencia después de la selección. Con `wrap`, vuelve a la primera.
pub fn next_match(matches: &[Range<usize>], selection: &Range<usize>, wrap: bool) -> Option<usize> {
    let key = (selection.start, selection.end);
    let index = matches.partition_point(|m| (m.start, m.end) <= key);
    if index < matches.len() {
        Some(index)
    } else if wrap && !matches.is_empty() {
        Some(0)
    } else {
        None
    }
}

/// Coincidencia anterior a la selección. Con `wrap`, vuelve a la última.
pub fn previous_match(
    matches: &[Range<usize>],
    selection: &Range<usize>,
    wrap: bool,
) -> Option<usize> {
    let key = (selection.start, selection.end);
    let index = matches.partition_point(|m| (m.start, m.end) < key);
    match index.checked_sub(1) {
        Some(previous) => Some(previous),
        None if wrap && !matches.is_empty() => Some(matches.len() - 1),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MATCHES: [Range<usize>; 3] = [2..4, 10..12, 20..22];

    #[test]
    fn next_from_cursor_and_from_match() {
        assert_eq!(next_match(&MATCHES, &(0..0), false), Some(0));
        assert_eq!(next_match(&MATCHES, &(2..2), false), Some(0));
        assert_eq!(next_match(&MATCHES, &(2..4), false), Some(1));
        assert_eq!(next_match(&MATCHES, &(21..21), false), None);
        assert_eq!(next_match(&MATCHES, &(21..21), true), Some(0));
    }

    #[test]
    fn previous_from_cursor_and_from_match() {
        assert_eq!(previous_match(&MATCHES, &(15..15), false), Some(1));
        assert_eq!(previous_match(&MATCHES, &(10..12), false), Some(0));
        assert_eq!(previous_match(&MATCHES, &(2..4), false), None);
        assert_eq!(previous_match(&MATCHES, &(2..4), true), Some(2));
    }

    #[test]
    fn exact_match_index() {
        assert_eq!(match_index(&MATCHES, &(10..12)), Some(1));
        assert_eq!(match_index(&MATCHES, &(10..11)), None);
        assert_eq!(match_index(&[], &(0..0)), None);
    }

    #[test]
    fn no_matches() {
        assert_eq!(next_match(&[], &(0..0), true), None);
        assert_eq!(previous_match(&[], &(0..0), true), None);
    }
}
