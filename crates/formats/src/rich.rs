//! Análisis de una línea con marcas de Markdown para mostrarla con formato (edición
//! WYSIWYG): tipo de bloque (párrafo, título, lista, tabla…) y estilo de cada carácter,
//! incluido si es una marca que no se dibuja (`**`, `#`, `[`, `](url)`…).
//!
//! Solo se reconoce lo que pone la barra de formato: títulos, listas, negrita, cursiva,
//! tachado, código entre comillas invertidas, vínculos y tablas.

use crate::markup::heading_level;

/// Tipo de bloque de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Block {
    Paragraph,
    /// Título de nivel 1 a 6.
    Heading(u8),
    /// Elemento de lista con viñeta; el guion se dibuja como «•».
    Bullet,
    /// Elemento de lista numerada (el número se ve tal cual).
    Numbered,
    /// Fila de tabla o valla de código: se muestra tal cual, en monoespaciada.
    Raw,
}

/// Estilo de un carácter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CharStyle {
    /// Marca que no se dibuja.
    pub hidden: bool,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
    pub link: bool,
    /// Guion de una lista que se dibuja como «•».
    pub bullet: bool,
}

/// Línea analizada: un estilo por carácter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMarkup {
    pub block: Block,
    pub chars: Vec<CharStyle>,
    /// Caracteres del principio que son marca de bloque (sangría incluida): el cursor no
    /// se coloca antes de ellos.
    pub prefix: usize,
}

impl LineMarkup {
    /// Rangos de caracteres ocultos seguidos (para que el cursor los salte).
    pub fn hidden_runs(&self) -> Vec<std::ops::Range<usize>> {
        let mut runs = Vec::new();
        let mut start = None;
        for (index, style) in self.chars.iter().enumerate() {
            match (style.hidden, start) {
                (true, None) => start = Some(index),
                (false, Some(from)) => {
                    runs.push(from..index);
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(from) = start {
            runs.push(from..self.chars.len());
        }
        runs
    }
}

impl LineMarkup {
    /// Caracteres visibles antes de la columna `col` (posición en pantalla).
    pub fn visible_offset(&self, col: usize) -> usize {
        self.chars
            .iter()
            .take(col)
            .filter(|style| !style.hidden)
            .count()
    }

    /// Columna válida para el cursor: nunca antes de la marca de bloque ni dentro de una
    /// marca oculta (va al final de la marca si `forward`, al principio si no).
    pub fn snap(&self, col: usize, forward: bool) -> usize {
        let col = col.min(self.chars.len());
        if col < self.prefix {
            return self.prefix;
        }
        for run in self.hidden_runs() {
            if run.start < col && col < run.end {
                return if forward || run.start < self.prefix {
                    run.end
                } else {
                    run.start
                };
            }
        }
        col
    }
}

/// Qué hacer al pulsar Intro al final de un elemento de lista.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListContinuation {
    /// Insertar este texto tras el salto (sangría y marca del siguiente elemento).
    Continue(String),
    /// El elemento está vacío: quitar su marca (los primeros `n` caracteres) y terminar
    /// la lista.
    End(usize),
}

/// Continuación de la lista de `line`, si es un elemento de lista.
pub fn continue_list(line: &str) -> Option<ListContinuation> {
    let markup = parse_line(line);
    let indent: String = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();
    let marker = match markup.block {
        Block::Bullet => {
            let symbol = line.chars().nth(indent.chars().count()).unwrap_or('-');
            format!("{symbol} ")
        }
        Block::Numbered => {
            let rest: String = line.chars().skip(indent.chars().count()).collect();
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let close = rest.chars().nth(digits.chars().count()).unwrap_or('.');
            let next = digits.parse::<u64>().map_or(1, |n| n.saturating_add(1));
            format!("{next}{close} ")
        }
        _ => return None,
    };
    if line.chars().count() <= markup.prefix
        || line.chars().skip(markup.prefix).all(char::is_whitespace)
    {
        Some(ListContinuation::End(markup.prefix))
    } else {
        Some(ListContinuation::Continue(format!("{indent}{marker}")))
    }
}

/// Analiza una línea (sin el salto).
pub fn parse_line(line: &str) -> LineMarkup {
    let chars: Vec<char> = line.chars().collect();
    let mut styles = vec![CharStyle::default(); chars.len()];
    let trimmed = line.trim_start();
    let indent = chars.len() - trimmed.chars().count();

    if trimmed.starts_with('|') || trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        return LineMarkup {
            block: Block::Raw,
            chars: styles,
            prefix: 0,
        };
    }

    let (block, prefix) = if let Some(level) = heading_level(trimmed) {
        // «## » se oculta entero (almohadillas y espacios que las siguen).
        let hashes = usize::from(level);
        let spaces = trimmed
            .chars()
            .skip(hashes)
            .take_while(|&c| c == ' ')
            .count();
        let end = indent + hashes + spaces;
        for style in styles.iter_mut().take(end) {
            style.hidden = true;
        }
        (Block::Heading(level), end)
    } else if let ['-' | '*' | '+', ' ', ..] = chars.get(indent..).unwrap_or_default() {
        if let Some(style) = styles.get_mut(indent) {
            style.bullet = true;
        }
        (Block::Bullet, indent + 2)
    } else {
        let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
        let after: Vec<char> = trimmed.chars().skip(digits).take(2).collect();
        if digits > 0 && matches!(after.as_slice(), ['.' | ')', ' ']) {
            (Block::Numbered, indent + digits + 2)
        } else {
            (Block::Paragraph, 0)
        }
    };

    inline_styles(&chars, prefix, &mut styles);
    LineMarkup {
        block,
        chars: styles,
        prefix,
    }
}

/// Marca los estilos dentro de la línea a partir de `from`.
fn inline_styles(chars: &[char], from: usize, styles: &mut [CharStyle]) {
    let mut used = vec![false; chars.len()];
    for item in used.iter_mut().take(from) {
        *item = true;
    }

    // Código: lo de dentro no se analiza más.
    let mut i = from;
    while i < chars.len() {
        if chars.get(i) == Some(&'`')
            && let Some(close) = find(chars, &used, i + 1, "`")
            && close > i + 1
        {
            hide(styles, &mut used, i, 1);
            hide(styles, &mut used, close, 1);
            for j in i + 1..close {
                if let Some(style) = styles.get_mut(j) {
                    style.code = true;
                }
                if let Some(slot) = used.get_mut(j) {
                    *slot = true;
                }
            }
            i = close + 1;
        } else {
            i += 1;
        }
    }

    // Vínculos: [texto](url) → texto.
    let mut i = from;
    while i < chars.len() {
        if chars.get(i) == Some(&'[')
            && !used.get(i).copied().unwrap_or(true)
            && let Some(close) = find(chars, &used, i + 1, "]")
            && chars.get(close + 1) == Some(&'(')
            && let Some(end) = find(chars, &used, close + 2, ")")
        {
            hide(styles, &mut used, i, 1);
            hide(styles, &mut used, close, end + 1 - close);
            for style in styles.iter_mut().take(close).skip(i + 1) {
                style.link = true;
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }

    pair(chars, &mut used, styles, "~~", |style| style.strike = true);
    pair(chars, &mut used, styles, "**", |style| style.bold = true);
    pair(chars, &mut used, styles, "*", |style| style.italic = true);
}

/// Primera aparición de `marker` desde `from` que no esté usada.
fn find(chars: &[char], used: &[bool], from: usize, marker: &str) -> Option<usize> {
    let marker: Vec<char> = marker.chars().collect();
    (from..chars.len()).find(|&i| {
        chars.get(i..i + marker.len()) == Some(marker.as_slice())
            && used
                .get(i..i + marker.len())
                .is_some_and(|slots| slots.iter().all(|used| !used))
    })
}

fn hide(styles: &mut [CharStyle], used: &mut [bool], at: usize, len: usize) {
    for j in at..at + len {
        if let Some(style) = styles.get_mut(j) {
            style.hidden = true;
        }
        if let Some(slot) = used.get_mut(j) {
            *slot = true;
        }
    }
}

/// Empareja aperturas y cierres de `marker` y aplica `apply` a lo de dentro. La apertura
/// no puede ir seguida de un espacio ni el cierre precedido de uno (como en Markdown).
fn pair(
    chars: &[char],
    used: &mut [bool],
    styles: &mut [CharStyle],
    marker: &str,
    apply: impl Fn(&mut CharStyle),
) {
    let len = marker.chars().count();
    let symbol = marker.chars().next().unwrap_or('*');
    // Un carácter de marca suelto pegado (p. ej. el segundo de un «**» sin pareja) no
    // puede ser el contenido: «a ** b ** c» no es cursiva.
    let blocks = |used: &[bool], index: usize| {
        chars.get(index).is_none_or(|c| {
            c.is_whitespace() || (*c == symbol && !used.get(index).copied().unwrap_or(true))
        })
    };
    let mut i = 0;
    while let Some(open) = find(chars, used, i, marker) {
        let content = open + len;
        if blocks(used, content) {
            i = open + 1;
            continue;
        }
        let close = (content + 1..chars.len())
            .find(|&j| find(chars, used, j, marker) == Some(j) && !blocks(used, j - 1));
        let Some(close) = close else {
            break;
        };
        hide(styles, used, open, len);
        hide(styles, used, close, len);
        for style in styles.iter_mut().take(close).skip(content) {
            apply(style);
        }
        i = close + len;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Texto visible y caracteres con un estilo.
    fn visible(line: &str) -> String {
        let markup = parse_line(line);
        line.chars()
            .zip(&markup.chars)
            .filter(|(_, style)| !style.hidden)
            .map(|(c, style)| if style.bullet { '•' } else { c })
            .collect()
    }

    fn styled(line: &str, test: impl Fn(&CharStyle) -> bool) -> String {
        let markup = parse_line(line);
        line.chars()
            .zip(&markup.chars)
            .filter(|(_, style)| test(style))
            .map(|(c, _)| c)
            .collect()
    }

    #[test]
    fn headings_hide_their_hashes() {
        let markup = parse_line("## Verde viento");
        assert_eq!(markup.block, Block::Heading(2));
        assert_eq!(markup.prefix, 3);
        assert_eq!(visible("## Verde viento"), "Verde viento");
        assert_eq!(parse_line("#hashtag").block, Block::Paragraph);
    }

    #[test]
    fn lists_keep_their_marker_visible() {
        assert_eq!(visible("- pan"), "• pan");
        assert_eq!(parse_line("  - pan").prefix, 4);
        let numbered = parse_line("12. doce");
        assert_eq!(numbered.block, Block::Numbered);
        assert_eq!(numbered.prefix, 4);
        assert_eq!(visible("12. doce"), "12. doce");
    }

    #[test]
    fn inline_styles_and_hidden_markers() {
        let line = "verde **que** te *quiero* ~~verde~~";
        assert_eq!(visible(line), "verde que te quiero verde");
        assert_eq!(styled(line, |s| s.bold), "que");
        assert_eq!(styled(line, |s| s.italic), "quiero");
        assert_eq!(styled(line, |s| s.strike), "verde");
        let both = "***todo***";
        assert_eq!(visible(both), "todo");
        assert_eq!(styled(both, |s| s.bold && s.italic), "todo");
    }

    #[test]
    fn code_and_links() {
        let line = "usa `**x**` y [la web](https://a.es)";
        assert_eq!(visible(line), "usa **x** y la web");
        assert_eq!(styled(line, |s| s.code), "**x**");
        assert_eq!(styled(line, |s| s.link), "la web");
    }

    #[test]
    fn unpaired_or_spaced_markers_are_text() {
        assert_eq!(visible("2 * 3 = 6"), "2 * 3 = 6");
        assert_eq!(visible("**abierta"), "**abierta");
        assert_eq!(visible("a ** b ** c"), "a ** b ** c");
    }

    #[test]
    fn tables_and_fences_are_raw() {
        assert_eq!(parse_line("| a | b |").block, Block::Raw);
        assert_eq!(visible("| **a** |"), "| **a** |");
        assert_eq!(parse_line("```rust").block, Block::Raw);
    }

    #[test]
    fn cursor_snaps_out_of_markers_and_prefixes() {
        let markup = parse_line("- a **b** c");
        // Antes de la viñeta, después de ella.
        assert_eq!(markup.snap(0, false), 2);
        // Entre los dos asteriscos: al final hacia delante, al principio hacia atrás.
        assert_eq!(markup.snap(5, true), 6);
        assert_eq!(markup.snap(5, false), 4);
        assert_eq!(markup.visible_offset(6), 4);
        let heading = parse_line("## Título");
        assert_eq!(heading.snap(1, false), 3);
    }

    #[test]
    fn lists_continue_on_enter() {
        assert_eq!(
            continue_list("  - pan"),
            Some(ListContinuation::Continue("  - ".to_owned()))
        );
        assert_eq!(
            continue_list("9. nueve"),
            Some(ListContinuation::Continue("10. ".to_owned()))
        );
        assert_eq!(continue_list("- "), Some(ListContinuation::End(2)));
        assert_eq!(continue_list("texto"), None);
    }

    #[test]
    fn hidden_runs_group_markers() {
        let markup = parse_line("a **b** c");
        assert_eq!(markup.hidden_runs(), vec![2..4, 5..7]);
    }
}
