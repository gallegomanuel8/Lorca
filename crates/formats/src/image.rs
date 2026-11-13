//! Imágenes en el texto con formato: una línea que solo contiene `![descripción](ruta)` se
//! muestra como la imagen, con la descripción debajo como pie.
//!
//! El ancho en pantalla se guarda como en Obsidian: `![descripción|300](ruta)` (en píxeles).
//! Otros visores muestran la imagen igual y solo ven `|300` en la descripción.

use std::ops::Range;

use crate::markup::MarkupEdit;

/// Imagen de una línea `![descripción](destino)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    /// Texto alternativo (se ve como pie), sin el ancho.
    pub alt: String,
    /// Columnas del texto alternativo en la línea (sin el ancho).
    pub alt_range: Range<usize>,
    /// Ancho en pantalla (`|300`), si lo tiene.
    pub width: Option<u32>,
    /// Columnas de todo lo que hay entre `![` y `]` (descripción y ancho).
    pub label_range: Range<usize>,
    /// Ruta o dirección tal como está escrita (sin `<>` ni título).
    pub target: String,
}

/// La imagen de `line`, si la línea solo contiene una (con espacios alrededor).
pub fn parse_image(line: &str) -> Option<ImageRef> {
    let chars: Vec<char> = line.chars().collect();
    let start = chars.iter().position(|c| !c.is_whitespace())?;
    let end = chars.iter().rposition(|c| !c.is_whitespace())? + 1;
    let body = chars.get(start..end)?;
    if body.get(..2)? != ['!', '['] || body.last() != Some(&')') {
        return None;
    }
    // Cierre del texto alternativo: el primer «](» (el texto no lleva corchetes).
    let close = body.windows(2).position(|pair| pair == [']', '('])?;
    let alt: String = body.get(2..close)?.iter().collect();
    if alt.contains('[') || alt.contains(']') {
        return None;
    }
    let inside: String = body.get(close + 2..body.len() - 1)?.iter().collect();
    let inside = inside.trim();
    let target = if let Some(rest) = inside.strip_prefix('<') {
        let (target, after) = rest.split_once('>')?;
        if !after.trim().is_empty() && !is_title(after.trim()) {
            return None;
        }
        target.to_owned()
    } else {
        // Sin `<>`: la ruta acaba en el primer espacio; lo que sigue tiene que ser título.
        let (target, after) = inside.split_once(' ').unwrap_or((inside, ""));
        if target.is_empty() || (!after.trim().is_empty() && !is_title(after.trim())) {
            return None;
        }
        target.to_owned()
    };
    let alt_start = start + 2;
    let label_range = alt_start..alt_start + alt.chars().count();
    let (alt, width) = split_width(&alt);
    Some(ImageRef {
        alt_range: alt_start..alt_start + alt.chars().count(),
        alt,
        width,
        label_range,
        target,
    })
}

/// Separa `descripción|300` en la descripción y el ancho.
fn split_width(label: &str) -> (String, Option<u32>) {
    if let Some((alt, width)) = label.rsplit_once('|')
        && !width.is_empty()
        && width.chars().all(|c| c.is_ascii_digit())
        && let Ok(width) = width.parse::<u32>()
    {
        return (alt.to_owned(), Some(width));
    }
    (label.to_owned(), None)
}

/// Cambia el ancho de la imagen de `line` (`None`: tamaño original). Devuelve la línea
/// nueva, o `None` si no es una imagen.
pub fn set_image_width(line: &str, width: Option<u32>) -> Option<String> {
    let image = parse_image(line)?;
    let label = match width {
        Some(width) => format!("{}|{width}", image.alt),
        None => image.alt,
    };
    let chars: Vec<char> = line.chars().collect();
    let before: String = chars.get(..image.label_range.start)?.iter().collect();
    let after: String = chars.get(image.label_range.end..)?.iter().collect();
    Some(format!("{before}{label}{after}"))
}

/// `line` con la imagen apuntando a `target` (conserva descripción, ancho y título).
/// `None` si la línea no es una imagen.
pub fn set_image_target(line: &str, target: &str) -> Option<String> {
    let image = parse_image(line)?;
    let chars: Vec<char> = line.chars().collect();
    // Lo que hay entre paréntesis empieza tras el primer «](» (como en `parse_image`).
    let open = chars.windows(2).position(|pair| pair == [']', '('])? + 2;
    let start = open
        + chars
            .get(open..)?
            .iter()
            .take_while(|c| c.is_whitespace())
            .count();
    let (start, end, written) = if chars.get(start) == Some(&'<') {
        let close = start + chars.get(start..)?.iter().position(|&c| c == '>')?;
        (start + 1, close, target.to_owned())
    } else {
        let len = image.target.chars().count();
        (start, start + len, markdown_target(target))
    };
    let before: String = chars.get(..start)?.iter().collect();
    let after: String = chars.get(end..)?.iter().collect();
    Some(format!("{before}{written}{after}"))
}

/// `"título"` o `'título'` (el título opcional de una imagen de Markdown).
fn is_title(text: &str) -> bool {
    text.len() >= 2
        && ((text.starts_with('"') && text.ends_with('"'))
            || (text.starts_with('\'') && text.ends_with('\'')))
}

/// Destino de imagen escrito para Markdown: entre `<>` si lleva espacios o paréntesis.
fn markdown_target(target: &str) -> String {
    if target.contains([' ', '(', ')']) {
        format!("<{target}>")
    } else {
        target.to_owned()
    }
}

/// Inserta las imágenes `images` (descripción, destino), cada una en su línea, donde está
/// el cursor. Si la línea tiene texto, van en líneas nuevas debajo; el cursor queda en la
/// línea siguiente a la última.
pub fn insert_images(
    text: &str,
    selection: Range<usize>,
    images: &[(String, String)],
) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let at = selection.end.min(chars.len());
    let line_start = chars
        .get(..at)
        .unwrap_or_default()
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |i| i + 1);
    let line_end = chars
        .get(at..)
        .unwrap_or_default()
        .iter()
        .position(|&c| c == '\n')
        .map_or(chars.len(), |i| at + i);
    let line_is_empty = chars
        .get(line_start..line_end)
        .unwrap_or_default()
        .iter()
        .all(|c| c.is_whitespace());
    let lines: Vec<String> = images
        .iter()
        .map(|(alt, target)| format!("![{alt}]({})", markdown_target(target)))
        .collect();
    let block = lines.join("\n");
    // En una línea vacía, la sustituye; si no, al final de la línea actual.
    let (range, prefix) = if line_is_empty {
        (line_start..line_end, "")
    } else {
        (line_end..line_end, "\n")
    };
    let has_next_line = line_end < chars.len();
    let suffix = if has_next_line { "" } else { "\n" };
    let inserted = format!("{prefix}{block}{suffix}");
    // El cursor, al principio de la línea que sigue a las imágenes.
    let cursor = range.start + prefix.chars().count() + block.chars().count() + 1;
    MarkupEdit {
        replacements: vec![(range, inserted)],
        selection: cursor..cursor,
    }
}

/// Mueve la línea `from` al hueco `to` (entre líneas: 0 = antes de la primera, el número
/// de líneas = después de la última). La selección resultante es la línea movida, con su
/// salto si lo tiene. `None` si no hay que mover nada.
pub fn move_line(text: &str, from: usize, to: usize) -> Option<MarkupEdit> {
    let lines: Vec<&str> = text.split('\n').collect();
    let count = lines.len();
    if from >= count || to > count || to == from || to == from + 1 {
        return None;
    }
    // Índice final de la línea movida.
    let target = if to > from { to - 1 } else { to };
    let (first, last) = (from.min(target), from.max(target));
    let mut region: Vec<&str> = lines.get(first..=last)?.to_vec();
    let moved = region.remove(from - first);
    region.insert(target - first, moved);
    let offset =
        |line: usize| -> usize { lines.iter().take(line).map(|l| l.chars().count() + 1).sum() };
    let start = offset(first);
    let end = offset(last) + lines.get(last)?.chars().count();
    let moved_start = start
        + region
            .iter()
            .take(target - first)
            .map(|l| l.chars().count() + 1)
            .sum::<usize>();
    let mut moved_end = moved_start + moved.chars().count();
    if target + 1 < count {
        moved_end += 1;
    }
    Some(MarkupEdit {
        replacements: vec![(start..end, region.join("\n"))],
        selection: moved_start..moved_end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(text: &str, edit: &MarkupEdit) -> String {
        let mut chars: Vec<char> = text.chars().collect();
        for (range, replacement) in edit.replacements.iter().rev() {
            chars.splice(range.clone(), replacement.chars());
        }
        chars.into_iter().collect()
    }

    #[test]
    fn image_target_is_replaced() {
        assert_eq!(
            set_image_target(
                "![gato|300](/tmp/a.png \"Mi gato\")",
                "notas_imagenes/a.png"
            )
            .as_deref(),
            Some("![gato|300](notas_imagenes/a.png \"Mi gato\")")
        );
        // Con espacios, entre `<>`; si ya lo estaba, se mantiene.
        assert_eq!(
            set_image_target("  ![x](/tmp/a.png)", "mis notas_imagenes/a.png").as_deref(),
            Some("  ![x](<mis notas_imagenes/a.png>)")
        );
        assert_eq!(
            set_image_target("![x](</tmp/una foto.png>)", "b.png").as_deref(),
            Some("![x](<b.png>)")
        );
        assert_eq!(set_image_target("texto", "b.png"), None);
    }

    #[test]
    fn moves_lines_up_and_down() {
        let text = "a\nimg\nb\nc";
        // Abajo, detrás de «b».
        let edit = move_line(text, 1, 3).unwrap();
        let result = apply(text, &edit);
        assert_eq!(result, "a\nb\nimg\nc");
        let selected: String = result
            .chars()
            .skip(edit.selection.start)
            .take(edit.selection.len())
            .collect();
        assert_eq!(selected, "img\n");
        // Arriba del todo.
        let edit = move_line(text, 1, 0).unwrap();
        assert_eq!(apply(text, &edit), "img\na\nb\nc");
        // Al final (la última línea no tiene salto).
        let edit = move_line(text, 1, 4).unwrap();
        let result = apply(text, &edit);
        assert_eq!(result, "a\nb\nc\nimg");
        assert_eq!(edit.selection, 6..9);
        // La última, hacia arriba.
        let edit = move_line(text, 3, 1).unwrap();
        assert_eq!(apply(text, &edit), "a\nc\nimg\nb");
        // Soltar en su propio sitio no hace nada.
        assert!(move_line(text, 1, 1).is_none());
        assert!(move_line(text, 1, 2).is_none());
        assert!(move_line(text, 9, 0).is_none());
    }

    #[test]
    fn reads_and_changes_the_width() {
        let image = parse_image("![Gato|320](g.png)").unwrap();
        assert_eq!(image.alt, "Gato");
        assert_eq!(image.alt_range, 2..6);
        assert_eq!(image.width, Some(320));
        // Una barra que no va seguida de un número es parte de la descripción.
        let image = parse_image("![a|b](g.png)").unwrap();
        assert_eq!((image.alt.as_str(), image.width), ("a|b", None));

        assert_eq!(
            set_image_width("![Gato](g.png)", Some(200)).as_deref(),
            Some("![Gato|200](g.png)")
        );
        assert_eq!(
            set_image_width("  ![Gato|320](<a b.png>)", None).as_deref(),
            Some("  ![Gato](<a b.png>)")
        );
        assert_eq!(
            set_image_width("![|90](g.png)", Some(150)).as_deref(),
            Some("![|150](g.png)")
        );
        assert_eq!(set_image_width("texto", Some(1)), None);
    }

    #[test]
    fn parses_an_image_line() {
        let image = parse_image("![Gato](notas_imagenes/gato.png)").unwrap();
        assert_eq!(image.alt, "Gato");
        assert_eq!(image.alt_range, 2..6);
        assert_eq!(image.target, "notas_imagenes/gato.png");
    }

    #[test]
    fn accepts_angle_brackets_titles_and_spaces_around() {
        let image = parse_image("  ![](<mis fotos/a (1).png> \"Título\")  ").unwrap();
        assert_eq!(image.alt, "");
        assert_eq!(image.alt_range, 4..4);
        assert_eq!(image.target, "mis fotos/a (1).png");
        let image = parse_image("![x](a.png 'título')").unwrap();
        assert_eq!(image.target, "a.png");
    }

    #[test]
    fn rejects_lines_with_more_than_an_image() {
        assert_eq!(parse_image("Mira ![x](a.png)"), None);
        assert_eq!(parse_image("![x](a.png) y más"), None);
        assert_eq!(parse_image("[x](a.png)"), None);
        assert_eq!(parse_image("![x]()"), None);
        assert_eq!(parse_image("![x](a b)"), None);
    }

    #[test]
    fn inserts_in_an_empty_line() {
        let text = "Hola\n\nAdiós";
        let edit = insert_images(text, 5..5, &[("gato".into(), "img/gato.png".into())]);
        let result = apply(text, &edit);
        assert_eq!(result, "Hola\n![gato](img/gato.png)\nAdiós");
        // El cursor, al principio de «Adiós».
        assert_eq!(edit.selection, 27..27);
        assert_eq!(result.chars().nth(edit.selection.start), Some('A'));
    }

    #[test]
    fn inserts_below_a_line_with_text_and_quotes_spaces() {
        let text = "Hola";
        let images = [
            ("a".to_owned(), "x/a b.png".to_owned()),
            ("b".to_owned(), "x/b.png".to_owned()),
        ];
        let edit = insert_images(text, 2..2, &images);
        let result = apply(text, &edit);
        assert_eq!(result, "Hola\n![a](<x/a b.png>)\n![b](x/b.png)\n");
        assert_eq!(edit.selection.start, result.chars().count());
        for line in result.lines().skip(1) {
            assert!(parse_image(line).is_some(), "{line}");
        }
    }
}
