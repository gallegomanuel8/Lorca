//! Formato con marcas de Markdown: negrita (`**`), cursiva (`*`) y listas (`- ` y
//! `1. `). Cada orden pone la marca o, si ya está, la quita.
//!
//! Trabajan con índices de carácter sobre el texto completo y devuelven las sustituciones
//! (ordenadas y sin solapes) y la selección resultante, para aplicarlas en una sola
//! transacción.

use std::ops::Range;

/// Formato dentro de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inline {
    Bold,
    Italic,
}

/// Tipo de lista.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListKind {
    Bullet,
    Numbered,
}

/// Cambios que aplicar: sustituciones (rango de caracteres → texto) y la selección final.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkupEdit {
    pub replacements: Vec<(Range<usize>, String)>,
    pub selection: Range<usize>,
}

impl Inline {
    fn marker(self) -> &'static str {
        match self {
            Self::Bold => "**",
            Self::Italic => "*",
        }
    }

    /// ¿Una racha de `stars` asteriscos contiene esta marca? `***` son las dos.
    fn present_in(self, stars: usize) -> bool {
        match self {
            Self::Bold => stars >= 2,
            Self::Italic => stars % 2 == 1,
        }
    }
}

/// Pone o quita la negrita o la cursiva de `selection`.
///
/// - Sin selección inserta las dos marcas con el cursor en medio.
/// - Si la selección ya está rodeada por la marca (por fuera o por dentro), la quita.
/// - Si no, la rodea con ella y deja seleccionado el mismo texto.
pub fn toggle_inline(text: &str, selection: Range<usize>, style: Inline) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let start = selection.start.min(chars.len());
    let end = selection.end.clamp(start, chars.len());
    let marker = style.marker();
    let m = marker.chars().count();

    if start == end {
        return MarkupEdit {
            replacements: vec![(start..start, marker.repeat(2))],
            selection: start + m..start + m,
        };
    }

    let stars_before = chars
        .get(..start)
        .unwrap_or_default()
        .iter()
        .rev()
        .take_while(|&&c| c == '*')
        .count();
    let stars_after = chars
        .get(end..)
        .unwrap_or_default()
        .iter()
        .take_while(|&&c| c == '*')
        .count();
    // Marcas alrededor de la selección: se quitan las más cercanas al texto.
    if style.present_in(stars_before) && style.present_in(stars_after) {
        return MarkupEdit {
            replacements: vec![
                (start - m..start, String::new()),
                (end..end + m, String::new()),
            ],
            selection: start - m..end - m,
        };
    }

    let inner = chars.get(start..end).unwrap_or_default();
    let leading = inner.iter().take_while(|&&c| c == '*').count();
    let trailing = inner.iter().rev().take_while(|&&c| c == '*').count();
    // Marcas dentro de la selección (se seleccionó «**texto**»).
    if leading + trailing < inner.len() && style.present_in(leading) && style.present_in(trailing) {
        return MarkupEdit {
            replacements: vec![
                (start + leading - m..start + leading, String::new()),
                (end - trailing..end - trailing + m, String::new()),
            ],
            selection: start..end - 2 * m,
        };
    }

    MarkupEdit {
        replacements: vec![
            (start..start, marker.to_owned()),
            (end..end, marker.to_owned()),
        ],
        selection: start + m..end + m,
    }
}

/// Convierte en lista (o deja de serlo) las líneas que toca `selection`.
///
/// Si todas las líneas con texto ya son de ese tipo de lista, se quita la marca; si no,
/// se pone en todas (una lista de otro tipo cambia de marca). Las numeradas se numeran
/// desde 1. La sangría se respeta y las líneas en blanco no se tocan. Al terminar queda
/// seleccionado el bloque de líneas.
pub fn toggle_list(text: &str, selection: Range<usize>, kind: ListKind) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let start = selection.start.min(chars.len());
    let mut end = selection.end.clamp(start, chars.len());
    // Una selección que acaba al principio de una línea no incluye esa línea.
    if end > start && chars.get(end - 1) == Some(&'\n') {
        end -= 1;
    }
    let first = chars
        .get(..start)
        .unwrap_or_default()
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |i| i + 1);
    let last = chars
        .get(end..)
        .unwrap_or_default()
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |i| end + i);

    // (inicio de la línea, sangría, marca de lista existente, ¿en blanco?)
    let mut lines = Vec::new();
    let mut line_start = first;
    loop {
        let line_end = chars
            .get(line_start..last)
            .unwrap_or_default()
            .iter()
            .position(|&c| c == '\n')
            .map_or(last, |i| line_start + i);
        let line = chars.get(line_start..line_end).unwrap_or_default();
        let indent = line
            .iter()
            .take_while(|c| **c == ' ' || **c == '\t')
            .count();
        let rest = line.get(indent..).unwrap_or_default();
        let blank = rest.is_empty();
        lines.push((line_start, indent, list_marker(rest), blank));
        if line_end >= last {
            break;
        }
        line_start = line_end + 1;
    }

    let all_marked = lines
        .iter()
        .filter(|(_, _, _, blank)| !blank)
        .all(|(_, _, marker, _)| marker.is_some_and(|(found, _)| found == kind));
    let any_text = lines.iter().any(|(_, _, _, blank)| !blank);

    let mut replacements = Vec::new();
    let mut number = 0;
    let mut delta: isize = 0;
    for (line_start, indent, marker, blank) in lines {
        if blank || !any_text {
            continue;
        }
        let at = line_start + indent;
        let old_len = marker.map_or(0, |(_, len)| len);
        let new_marker = if all_marked {
            String::new()
        } else {
            number += 1;
            match kind {
                ListKind::Bullet => "- ".to_owned(),
                ListKind::Numbered => format!("{number}. "),
            }
        };
        delta += new_marker.chars().count() as isize - old_len as isize;
        replacements.push((at..at + old_len, new_marker));
    }
    let new_last = last.saturating_add_signed(delta);
    MarkupEdit {
        replacements,
        selection: first..new_last,
    }
}

/// Marca de lista al principio de `rest` (ya sin sangría): tipo y longitud con el espacio.
fn list_marker(rest: &[char]) -> Option<(ListKind, usize)> {
    match rest {
        ['-' | '*' | '+', ' ', ..] => Some((ListKind::Bullet, 2)),
        _ => {
            let digits = rest.iter().take_while(|c| c.is_ascii_digit()).count();
            let after = rest.get(digits..).unwrap_or_default();
            (digits > 0 && matches!(after, ['.' | ')', ' ', ..]))
                .then_some((ListKind::Numbered, digits + 2))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Aplica el resultado y devuelve el texto y lo seleccionado.
    fn apply(text: &str, edit: &MarkupEdit) -> (String, String) {
        let mut chars: Vec<char> = text.chars().collect();
        for (range, replacement) in edit.replacements.iter().rev() {
            chars.splice(range.clone(), replacement.chars());
        }
        let selected = chars[edit.selection.clone()].iter().collect();
        (chars.into_iter().collect(), selected)
    }

    fn inline(text: &str, selection: Range<usize>, style: Inline) -> (String, String) {
        apply(text, &toggle_inline(text, selection, style))
    }

    #[test]
    fn bold_wraps_and_unwraps() {
        let (text, selected) = inline("verde que te", 6..9, Inline::Bold);
        assert_eq!(text, "verde **que** te");
        assert_eq!(selected, "que");
        // Otra vez con la misma selección: se quita.
        let (text, selected) = inline("verde **que** te", 8..11, Inline::Bold);
        assert_eq!(text, "verde que te");
        assert_eq!(selected, "que");
        // Seleccionando también las marcas.
        let (text, selected) = inline("verde **que** te", 6..13, Inline::Bold);
        assert_eq!(text, "verde que te");
        assert_eq!(selected, "que");
    }

    #[test]
    fn italic_and_bold_do_not_mix_up() {
        // En «**que**», la cursiva no ve sus marcas: añade las suyas.
        let (text, _) = inline("**que**", 2..5, Inline::Italic);
        assert_eq!(text, "***que***");
        let (text, selected) = inline("***que***", 3..6, Inline::Italic);
        assert_eq!(text, "**que**");
        assert_eq!(selected, "que");
        let (text, _) = inline("***que***", 3..6, Inline::Bold);
        assert_eq!(text, "*que*");
        let (text, _) = inline("*que*", 1..4, Inline::Italic);
        assert_eq!(text, "que");
    }

    #[test]
    fn empty_selection_inserts_markers_around_the_cursor() {
        let edit = toggle_inline("ab", 1..1, Inline::Bold);
        let (text, _) = apply("ab", &edit);
        assert_eq!(text, "a****b");
        assert_eq!(edit.selection, 3..3);
    }

    fn list(text: &str, selection: Range<usize>, kind: ListKind) -> (String, String) {
        apply(text, &toggle_list(text, selection, kind))
    }

    #[test]
    fn bullets_on_and_off() {
        let (text, selected) = list("pan\nleche\n\nhuevos\nfin", 1..13, ListKind::Bullet);
        assert_eq!(text, "- pan\n- leche\n\n- huevos\nfin");
        assert_eq!(selected, "- pan\n- leche\n\n- huevos");
        let (text, _) = list("- pan\n- leche", 0..13, ListKind::Bullet);
        assert_eq!(text, "pan\nleche");
    }

    #[test]
    fn numbered_lists_renumber_and_replace_bullets() {
        let (text, _) = list("- uno\n  dos\n7. tres", 0..20, ListKind::Numbered);
        assert_eq!(text, "1. uno\n  2. dos\n3. tres");
        let (text, _) = list("1. uno\n2. dos", 0..13, ListKind::Numbered);
        assert_eq!(text, "uno\ndos");
        let (text, _) = list("1. uno\n2. dos", 0..13, ListKind::Bullet);
        assert_eq!(text, "- uno\n- dos");
    }

    #[test]
    fn list_uses_the_cursor_line_and_ignores_a_trailing_line_start() {
        let (text, _) = list("a\nb\nc", 2..2, ListKind::Bullet);
        assert_eq!(text, "a\n- b\nc");
        // «a\n» seleccionado entero: solo la línea «a».
        let (text, _) = list("a\nb", 0..2, ListKind::Bullet);
        assert_eq!(text, "- a\nb");
        let (text, _) = list("", 0..0, ListKind::Bullet);
        assert_eq!(text, "");
    }
}
