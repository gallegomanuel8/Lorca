//! Formato con marcas de Markdown, con las mismas opciones que el Bloc de notas de
//! Windows 11: encabezados (`#`), negrita (`**`), cursiva (`*`), tachado (`~~`), listas
//! (`- ` y `1. `), vínculos (`[texto](url)`) y borrar el formato. Las órdenes de estilo
//! ponen la marca o, si ya está, la quitan.
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
    Strikethrough,
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
            Self::Strikethrough => "~~",
        }
    }

    /// Carácter de la marca.
    fn symbol(self) -> char {
        match self {
            Self::Bold | Self::Italic => '*',
            Self::Strikethrough => '~',
        }
    }

    /// ¿Una racha de `run` caracteres de la marca la contiene? `***` son negrita y cursiva.
    fn present_in(self, run: usize) -> bool {
        match self {
            Self::Bold | Self::Strikethrough => run >= 2,
            Self::Italic => run % 2 == 1,
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

    let symbol = style.symbol();
    let stars_before = chars
        .get(..start)
        .unwrap_or_default()
        .iter()
        .rev()
        .take_while(|&&c| c == symbol)
        .count();
    let stars_after = chars
        .get(end..)
        .unwrap_or_default()
        .iter()
        .take_while(|&&c| c == symbol)
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
    let leading = inner.iter().take_while(|&&c| c == symbol).count();
    let trailing = inner.iter().rev().take_while(|&&c| c == symbol).count();
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
    let Range {
        start: first,
        end: last,
    } = line_block(&chars, selection);

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

/// Primera y última posición (carácter) de las líneas que toca `selection`: desde el
/// principio de la primera hasta el final (sin salto) de la última. Una selección que
/// acaba al principio de una línea no incluye esa línea.
fn line_block(chars: &[char], selection: Range<usize>) -> Range<usize> {
    let start = selection.start.min(chars.len());
    let mut end = selection.end.clamp(start, chars.len());
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
    first..last
}

/// Nivel de encabezado (1 a 6) de una línea, si lo es: `## Título` → 2.
pub fn heading_level(line: &str) -> Option<u8> {
    let hashes = line.chars().take_while(|&c| c == '#').count();
    let rest = line.get(hashes..).unwrap_or_default();
    ((1..=6).contains(&hashes) && (rest.is_empty() || rest.starts_with(' ')))
        .then(|| u8::try_from(hashes).unwrap_or(1))
}

/// Convierte las líneas que toca `selection` en encabezados de nivel `level` (1 a 6) o,
/// con `None`, en texto normal. Sustituye el nivel anterior y la marca de lista, si
/// la había.
pub fn set_heading(text: &str, selection: Range<usize>, level: Option<u8>) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let block = line_block(&chars, selection);
    let prefix = level.map_or(String::new(), |level| {
        format!("{} ", "#".repeat(usize::from(level.clamp(1, 6))))
    });
    let mut replacements = Vec::new();
    let mut delta: isize = 0;
    let mut line_start = block.start;
    loop {
        let line_end = chars
            .get(line_start..block.end)
            .unwrap_or_default()
            .iter()
            .position(|&c| c == '\n')
            .map_or(block.end, |i| line_start + i);
        let line: String = chars
            .get(line_start..line_end)
            .unwrap_or_default()
            .iter()
            .collect();
        let rest = line.trim_start();
        if !rest.is_empty() || level.is_none() {
            // Longitud de lo que se quita: sangría, almohadillas o marca de lista y espacio.
            let old: usize = match heading_level(rest) {
                Some(hashes) => {
                    let after = rest.get(usize::from(hashes)..).unwrap_or_default();
                    usize::from(hashes) + after.len() - after.trim_start().len()
                }
                None => {
                    let rest_chars: Vec<char> = rest.chars().collect();
                    list_marker(&rest_chars).map_or(0, |(_, len)| len)
                }
            };
            let indent = line.chars().count() - rest.chars().count();
            let old = indent + old;
            if old > 0 || !prefix.is_empty() {
                replacements.push((line_start..line_start + old, prefix.clone()));
                delta += prefix.chars().count() as isize - old as isize;
            }
        }
        if line_end >= block.end {
            break;
        }
        line_start = line_end + 1;
    }
    MarkupEdit {
        replacements,
        selection: block.start..block.end.saturating_add_signed(delta),
    }
}

/// Vínculo: la selección pasa a ser el texto de `[texto](https://)` y queda seleccionada
/// la dirección para escribirla o pegarla encima. Sin selección, `[texto](https://)` con
/// «texto» seleccionado.
pub fn insert_link(text: &str, selection: Range<usize>) -> MarkupEdit {
    const URL: &str = "https://";
    let len = text.chars().count();
    let start = selection.start.min(len);
    let end = selection.end.clamp(start, len);
    if start == end {
        return MarkupEdit {
            replacements: vec![(start..start, format!("[texto]({URL})"))],
            selection: start + 1..start + 6,
        };
    }
    let url_start = end + 3;
    MarkupEdit {
        replacements: vec![
            (start..start, "[".to_owned()),
            (end..end, format!("]({URL})")),
        ],
        selection: url_start..url_start + URL.len(),
    }
}

/// Borra el formato de las líneas que toca `selection`: encabezados, marcas de lista,
/// `**`, `*`, `~~`, `` ` `` y vínculos (se queda el texto). Respeta la sangría.
pub fn clear_formatting(text: &str, selection: Range<usize>) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let block = line_block(&chars, selection);
    let original: String = chars
        .get(block.clone())
        .unwrap_or_default()
        .iter()
        .collect();
    let cleaned = original
        .split('\n')
        .map(clean_line)
        .collect::<Vec<_>>()
        .join("\n");
    let new_len = cleaned.chars().count();
    let replacements = if cleaned == original {
        Vec::new()
    } else {
        vec![(block.clone(), cleaned)]
    };
    MarkupEdit {
        replacements,
        selection: block.start..block.start + new_len,
    }
}

fn clean_line(line: &str) -> String {
    let rest = line.trim_start();
    let indent = &line[..line.len() - rest.len()];
    let rest = match heading_level(rest) {
        Some(hashes) => rest
            .get(usize::from(hashes)..)
            .unwrap_or_default()
            .trim_start(),
        None => {
            let rest_chars: Vec<char> = rest.chars().collect();
            let skip = list_marker(&rest_chars).map_or(0, |(_, len)| len);
            let byte = rest.char_indices().nth(skip).map_or(rest.len(), |(i, _)| i);
            rest.get(byte..).unwrap_or_default()
        }
    };
    let mut out = String::from(indent);
    let chars: Vec<char> = rest.chars().collect();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '*' | '`' => i += 1,
            '~' if chars.get(i + 1) == Some(&'~') => i += 2,
            // [texto](url) → texto
            '[' => {
                let close = chars
                    .get(i..)
                    .and_then(|tail| tail.iter().position(|&c| c == ']'));
                let link = close.and_then(|close| {
                    let paren = i + close + 1;
                    (chars.get(paren) == Some(&'('))
                        .then(|| chars.get(paren..)?.iter().position(|&c| c == ')'))
                        .flatten()
                        .map(|end| (i + close, paren + end))
                });
                match link {
                    Some((text_end, link_end)) => {
                        out.extend(
                            chars
                                .get(i + 1..text_end)
                                .unwrap_or_default()
                                .iter()
                                .filter(|&&c| c != '*' && c != '`'),
                        );
                        i = link_end + 1;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// ¿Es `line` un elemento de lista (con o sin sangría)?
pub fn is_list_item(line: &str) -> bool {
    let rest: Vec<char> = line.trim_start().chars().collect();
    list_marker(&rest).is_some()
}

/// Aumenta (o reduce) la sangría de los elementos de lista que toca `selection` en
/// `unit` (lo que inserta la tecla Tab). Las demás líneas no cambian.
pub fn indent_list(text: &str, selection: Range<usize>, unit: &str, increase: bool) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let block = line_block(&chars, selection.clone());
    let unit_len = unit.chars().count().max(1);
    let mut replacements = Vec::new();
    let mut delta: isize = 0;
    let mut line_start = block.start;
    loop {
        let line_end = chars
            .get(line_start..block.end)
            .unwrap_or_default()
            .iter()
            .position(|&c| c == '\n')
            .map_or(block.end, |i| line_start + i);
        let line: String = chars
            .get(line_start..line_end)
            .unwrap_or_default()
            .iter()
            .collect();
        if is_list_item(&line) {
            if increase {
                replacements.push((line_start..line_start, unit.to_owned()));
                delta += unit_len as isize;
            } else {
                // Se quita un tabulador o hasta `unit_len` espacios.
                let removable = match line.chars().next() {
                    Some('\t') => 1,
                    _ => line.chars().take_while(|&c| c == ' ').count().min(unit_len),
                };
                if removable > 0 {
                    replacements.push((line_start..line_start + removable, String::new()));
                    delta -= removable as isize;
                }
            }
        }
        if line_end >= block.end {
            break;
        }
        line_start = line_end + 1;
    }
    let first_shift = replacements
        .first()
        .filter(|(range, _)| range.start == block.start)
        .map_or(0, |(range, text)| {
            text.chars().count() as isize - range.len() as isize
        });
    MarkupEdit {
        replacements,
        selection: selection
            .start
            .saturating_add_signed(first_shift)
            .max(block.start)..selection.end.saturating_add_signed(delta),
    }
}

/// Inserta una tabla de `columns` columnas y `rows` filas (la primera es la cabecera) en
/// una línea propia, con el cursor en la primera celda.
pub fn insert_table(
    text: &str,
    selection: Range<usize>,
    rows: usize,
    columns: usize,
) -> MarkupEdit {
    let chars: Vec<char> = text.chars().collect();
    let at = selection.end.min(chars.len());
    let rows = rows.max(1);
    let columns = columns.max(1);
    let row = format!("|{}", "   |".repeat(columns));
    let separator = format!("|{}", " --- |".repeat(columns));
    let mut table = vec![row.clone(), separator];
    table.extend(std::iter::repeat_n(row, rows - 1));
    // En una línea propia, con una línea en blanco antes si hay texto.
    let line_start = chars
        .get(..at)
        .unwrap_or_default()
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |i| i + 1);
    let prefix = if at == line_start && (at == 0 || chars.get(at.saturating_sub(2)) == Some(&'\n'))
    {
        ""
    } else if at == line_start {
        "\n"
    } else {
        "\n\n"
    };
    let suffix = if chars.get(at).is_some() { "\n" } else { "" };
    let inserted = format!("{prefix}{}\n{suffix}", table.join("\n"));
    let cursor = at + prefix.chars().count() + 2;
    MarkupEdit {
        replacements: vec![(at..at, inserted)],
        selection: cursor..cursor,
    }
}

/// Cambios en la tabla donde está el cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableEdit {
    InsertRow,
    InsertColumn,
    DeleteRow,
    DeleteColumn,
}

/// Tabla que contiene `index`: rango de líneas (caracteres), filas con sus celdas, fila y
/// columna del cursor.
struct TableAt {
    block: Range<usize>,
    rows: Vec<Vec<String>>,
    row: usize,
    column: usize,
}

fn is_table_line(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells.iter().all(|cell| {
            let cell = cell.trim();
            !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':'))
        })
}

fn table_at(text: &str, index: usize) -> Option<TableAt> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut offset = 0;
    let mut cursor_line = None;
    let mut starts = Vec::with_capacity(lines.len());
    for (number, line) in lines.iter().enumerate() {
        starts.push(offset);
        let len = line.chars().count();
        if cursor_line.is_none() && index <= offset + len {
            cursor_line = Some((number, index - offset));
        }
        offset += len + 1;
    }
    let (line, column_chars) = cursor_line?;
    if !is_table_line(lines.get(line)?) {
        return None;
    }
    let mut first = line;
    while first > 0 && lines.get(first - 1).is_some_and(|l| is_table_line(l)) {
        first -= 1;
    }
    let mut last = line;
    while lines.get(last + 1).is_some_and(|l| is_table_line(l)) {
        last += 1;
    }
    let cells = |line: &str| -> Vec<String> {
        let inner = line.trim();
        let inner = inner.strip_prefix('|').unwrap_or(inner);
        let inner = inner.strip_suffix('|').unwrap_or(inner);
        inner
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect()
    };
    let rows: Vec<Vec<String>> = (first..=last)
        .filter_map(|n| lines.get(n))
        .map(|l| cells(l))
        .collect();
    let current = lines.get(line)?;
    let before: String = current.chars().take(column_chars).collect();
    let column = before.trim_start().matches('|').count().saturating_sub(1);
    let block_start = *starts.get(first)?;
    let block_end = starts.get(last)? + lines.get(last)?.chars().count();
    Some(TableAt {
        block: block_start..block_end,
        rows,
        row: line - first,
        column,
    })
}

/// `true` si `index` está dentro de una tabla de Markdown.
pub fn in_table(text: &str, index: usize) -> bool {
    table_at(text, index).is_some()
}

/// Inserta o borra una fila o columna de la tabla donde está el cursor. `None` si no está
/// en una tabla o el cambio no tiene sentido (borrar la cabecera o la única columna). La
/// tabla se reescribe con las celdas separadas por ` | ` y el cursor queda en la celda
/// nueva (o en la de al lado de la borrada).
pub fn edit_table(text: &str, index: usize, edit: TableEdit) -> Option<MarkupEdit> {
    let table = table_at(text, index)?;
    let mut rows = table.rows;
    let columns = rows.iter().map(Vec::len).max()?.max(1);
    for row in &mut rows {
        row.resize(columns, String::new());
    }
    let separator = rows.iter().position(|row| is_separator_row(row));
    let mut row = table.row;
    let mut column = table.column.min(columns - 1);
    match edit {
        TableEdit::InsertRow => {
            // Debajo de la fila actual (de la cabecera, debajo del separador).
            let below = match separator {
                Some(sep) if row <= sep => sep + 1,
                _ => row + 1,
            };
            rows.insert(below, vec![String::new(); columns]);
            row = below;
        }
        TableEdit::InsertColumn => {
            for (index, cells) in rows.iter_mut().enumerate() {
                let cell = if Some(index) == separator {
                    "---".to_owned()
                } else {
                    String::new()
                };
                cells.insert(column + 1, cell);
            }
            column += 1;
        }
        TableEdit::DeleteRow => {
            if Some(row) == separator || row == 0 || rows.len() <= 2 {
                return None;
            }
            rows.remove(row);
            row = row.min(rows.len() - 1);
        }
        TableEdit::DeleteColumn => {
            if columns < 2 {
                return None;
            }
            for cells in &mut rows {
                cells.remove(column);
            }
            column = column.min(columns - 2);
        }
    }
    let lines: Vec<String> = rows
        .iter()
        .map(|cells| format!("| {} |", cells.join(" | ")))
        .collect();
    // Cursor al principio del contenido de la celda (fila `row`, columna `column`).
    let before: usize = lines.iter().take(row).map(|l| l.chars().count() + 1).sum();
    let in_line: usize = rows.get(row).map_or(0, |cells| {
        cells
            .iter()
            .take(column)
            .map(|c| c.chars().count() + 3)
            .sum::<usize>()
    }) + 2;
    let cursor = table.block.start + before + in_line;
    Some(MarkupEdit {
        replacements: vec![(table.block, lines.join("\n"))],
        selection: cursor..cursor,
    })
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

    #[test]
    fn strikethrough_toggles() {
        let (text, selected) = inline("viento", 0..6, Inline::Strikethrough);
        assert_eq!(text, "~~viento~~");
        assert_eq!(selected, "viento");
        let (text, _) = inline("~~viento~~", 2..8, Inline::Strikethrough);
        assert_eq!(text, "viento");
    }

    #[test]
    fn headings_replace_levels_and_list_markers() {
        let heading =
            |text: &str, selection, level| apply(text, &set_heading(text, selection, level));
        assert_eq!(
            heading("Título\ntexto", 0..0, Some(2)).0,
            "## Título\ntexto"
        );
        assert_eq!(heading("## Título", 3..3, Some(1)).0, "# Título");
        assert_eq!(heading("### Título", 3..3, None).0, "Título");
        assert_eq!(heading("- punto", 0..0, Some(3)).0, "### punto");
        assert_eq!(heading("uno\n\ndos", 0..8, Some(1)).0, "# uno\n\n# dos");
        assert_eq!(heading_level("## a"), Some(2));
        assert_eq!(heading_level("#hashtag"), None);
        assert_eq!(heading_level("####### siete"), None);
    }

    #[test]
    fn links_wrap_the_selection_and_select_the_url() {
        let (text, selected) = apply("ver web", &insert_link("ver web", 4..7));
        assert_eq!(text, "ver [web](https://)");
        assert_eq!(selected, "https://");
        let (text, selected) = apply("", &insert_link("", 0..0));
        assert_eq!(text, "[texto](https://)");
        assert_eq!(selected, "texto");
    }

    #[test]
    fn clear_formatting_keeps_the_words() {
        let text = "## **Verde** que\n- te *quiero* ~~verde~~\n  1. [viento](https://a.es) `rama`";
        let (cleaned, selected) = apply(text, &clear_formatting(text, 0..text.chars().count()));
        assert_eq!(cleaned, "Verde que\nte quiero verde\n  viento rama");
        assert_eq!(selected, cleaned);
        // Sin formato no cambia nada.
        assert!(clear_formatting("hola", 0..4).replacements.is_empty());
    }

    #[test]
    fn list_indent_only_touches_list_items() {
        let text = "- uno\ntexto\n- dos";
        let (indented, _) = apply(text, &indent_list(text, 0..text.len(), "    ", true));
        assert_eq!(indented, "    - uno\ntexto\n    - dos");
        let (back, _) = apply(
            &indented,
            &indent_list(&indented, 0..indented.len(), "    ", false),
        );
        assert_eq!(back, text);
        assert!(is_list_item("  1. a"));
        assert!(!is_list_item("texto"));
    }

    #[test]
    fn tables_are_inserted_on_their_own_line() {
        let edit = insert_table("hola", 4..4, 2, 3);
        let (text, _) = apply("hola", &edit);
        assert_eq!(
            text,
            "hola\n\n|   |   |   |\n| --- | --- | --- |\n|   |   |   |\n"
        );
        assert_eq!(edit.selection, 8..8);
        let (text, _) = apply("", &insert_table("", 0..0, 1, 1));
        assert_eq!(text, "|   |\n| --- |\n");
    }

    #[test]
    fn table_rows_and_columns() {
        let text = "| a | b |\n| --- | --- |\n| 1 | 2 |";
        let in_one = text.find('1').unwrap();
        assert!(in_table(text, in_one));
        assert!(!in_table("texto", 2));
        let (rows, _) = apply(
            text,
            &edit_table(text, in_one, TableEdit::InsertRow).unwrap(),
        );
        assert_eq!(rows, "| a | b |\n| --- | --- |\n| 1 | 2 |\n|  |  |");
        let (columns, _) = apply(
            text,
            &edit_table(text, in_one, TableEdit::InsertColumn).unwrap(),
        );
        assert_eq!(columns, "| a |  | b |\n| --- | --- | --- |\n| 1 |  | 2 |");
        let (deleted, _) = apply(
            text,
            &edit_table(text, in_one, TableEdit::DeleteColumn).unwrap(),
        );
        assert_eq!(deleted, "| b |\n| --- |\n| 2 |");
        // La cabecera no se borra.
        assert!(edit_table(text, 2, TableEdit::DeleteRow).is_none());
        let three = format!("{text}\n| 3 | 4 |");
        let (fewer, _) = apply(
            &three,
            &edit_table(&three, in_one, TableEdit::DeleteRow).unwrap(),
        );
        assert_eq!(fewer, "| a | b |\n| --- | --- |\n| 3 | 4 |");
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
