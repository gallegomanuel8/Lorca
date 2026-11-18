//! Análisis de una línea con marcas de Markdown para mostrarla con formato (edición
//! WYSIWYG): tipo de bloque (párrafo, título, lista, tabla…) y estilo de cada carácter,
//! incluido si es una marca que no se dibuja (`**`, `#`, `[`, `](url)`…).
//!
//! Solo se reconoce lo que pone la barra de formato: títulos, listas, negrita, cursiva,
//! tachado, código entre comillas invertidas, vínculos, tablas e imágenes (una por línea).

use crate::image::parse_image;
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
    /// Tarea (`- [ ] texto`, hecha con `- [x]`): se ve una casilla que se marca con un clic.
    Task(bool),
    /// Fila de tabla o valla de código: se muestra tal cual, en monoespaciada.
    Raw,
    /// Línea dentro de un bloque de código con vallas: tal cual, en monoespaciada y con el
    /// resaltado de su lenguaje. Ver [`CodeFences`].
    Code,
    /// Línea con solo una imagen (`![descripción](ruta)`): se ve la imagen y, como pie, la
    /// descripción (lo único visible del texto). Ver [`parse_image`].
    Image,
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
    /// Corchete de una tarea que se dibuja como su casilla («☐» o «☑»).
    pub checkbox: bool,
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

impl LineMarkup {
    /// Formato de `range` (columnas de la línea) para marcar los botones de la barra: con
    /// selección, lo que tienen todos sus caracteres visibles; sin ella, el del carácter
    /// visible anterior (el que se continúa al escribir) o, al principio, el siguiente.
    /// `None` si no hay ningún carácter visible que mirar.
    pub fn style_in(&self, range: std::ops::Range<usize>) -> Option<CharStyle> {
        let visible = |index: &usize| {
            *index >= self.prefix && self.chars.get(*index).is_some_and(|style| !style.hidden)
        };
        let end = range.end.min(self.chars.len());
        let start = range.start.min(end);
        if start < end {
            let mut chars = (start..end)
                .filter(visible)
                .filter_map(|i| self.chars.get(i));
            let first = *chars.next()?;
            return Some(chars.fold(first, |all, style| CharStyle {
                bold: all.bold && style.bold,
                italic: all.italic && style.italic,
                strike: all.strike && style.strike,
                code: all.code && style.code,
                link: all.link && style.link,
                ..CharStyle::default()
            }));
        }
        let index = (0..start)
            .rev()
            .find(visible)
            .or_else(|| (start..self.chars.len()).find(visible));
        index
            .and_then(|i| self.chars.get(i))
            .map(|style| CharStyle {
                hidden: false,
                bullet: false,
                ..*style
            })
    }
}

/// ¿Acaba de escribirse formato de Markdown? `before` es la línea hasta el cursor, justo
/// después de teclear: un título (`# `), una viñeta (`- `, `* `, `+ `), un número (`1. `)
/// recién empezados, o una negrita (`**texto**`) o un tachado (`~~texto~~`) recién
/// cerrados. Sirve para que un texto plano nuevo pase a texto con formato al escribirlo.
pub fn typed_markdown(before: &str) -> bool {
    let rest = before.trim_start_matches([' ', '\t']);
    let Some(marker) = rest.strip_suffix(' ') else {
        return closed_inline(before, "**") || closed_inline(before, "~~");
    };
    let hashes = marker.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && hashes == marker.chars().count() {
        return true;
    }
    if matches!(marker, "-" | "*" | "+") {
        return true;
    }
    let digits = marker.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && digits < 4 && matches!(marker.get(digits..), Some(".") | Some(")"))
}

/// ¿Acaba `before` en `mark` cerrando un tramo `mark` + texto + `mark` (sin espacios junto
/// a las marcas)?
fn closed_inline(before: &str, mark: &str) -> bool {
    let Some(head) = before.strip_suffix(mark) else {
        return false;
    };
    let symbol = mark.chars().next().unwrap_or('*');
    if head.ends_with(symbol) || head.ends_with(char::is_whitespace) {
        return false;
    }
    let Some(open) = head.rfind(mark) else {
        return false;
    };
    let inner = &head[open + mark.len()..];
    let before_open = &head[..open];
    !inner.is_empty()
        && !inner.starts_with(char::is_whitespace)
        && !inner.starts_with(symbol)
        && !before_open.ends_with(symbol)
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
        Block::Task(_) => {
            let symbol = line.chars().nth(indent.chars().count()).unwrap_or('-');
            format!("{symbol} [ ] ")
        }
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

/// Si `rest` (sin sangría) empieza por una tarea (`- [ ] `, `- [x] `), si está hecha.
fn task_state(rest: &[char]) -> Option<bool> {
    match rest {
        [
            '-' | '*' | '+',
            ' ',
            '[',
            mark @ (' ' | 'x' | 'X'),
            ']',
            ' ',
            ..,
        ] => Some(*mark != ' '),
        _ => None,
    }
}

/// Marca o desmarca la tarea de `line`. Devuelve la línea nueva, o `None` si no es una
/// tarea.
pub fn toggle_task(line: &str) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    let indent = chars
        .iter()
        .take_while(|c| **c == ' ' || **c == '\t')
        .count();
    let done = task_state(chars.get(indent..)?)?;
    let mut out = chars;
    *out.get_mut(indent + 3)? = if done { ' ' } else { 'x' };
    Some(out.into_iter().collect())
}

/// Sigue los bloques de código con vallas (```` ``` ```` o `~~~`) línea a línea: dice qué
/// líneas son contenido de un bloque (las vallas no lo son). Hay que pasarle todas las
/// líneas en orden desde la primera.
#[derive(Debug, Clone, Copy, Default)]
pub struct CodeFences {
    open: Option<char>,
}

impl CodeFences {
    /// ¿Es `line` (la siguiente) contenido de un bloque de código?
    pub fn next_line(&mut self, line: &str) -> bool {
        let trimmed = line.trim_start();
        let fence = ['`', '~']
            .into_iter()
            .find(|&c| trimmed.starts_with(&String::from(c).repeat(3)));
        match (self.open, fence) {
            (Some(open), Some(fence)) if open == fence => {
                self.open = None;
                false
            }
            (Some(_), _) => true,
            (None, Some(fence)) => {
                self.open = Some(fence);
                false
            }
            (None, None) => false,
        }
    }
}

/// Análisis de `line` con `text` escrito en la columna `at`, si lo escrito se ve y todo lo
/// que ya se veía (`markup`, el análisis de `line`) conserva su estilo.
pub(crate) fn insertion_keeps_styles(
    line: &str,
    markup: &LineMarkup,
    at: usize,
    text: &str,
) -> Option<LineMarkup> {
    let chars: Vec<char> = line.chars().collect();
    let added = text.chars().count();
    let mut candidate: String = chars.iter().take(at).collect();
    candidate.push_str(text);
    candidate.extend(chars.iter().skip(at));
    let after = parse_line(&candidate);
    let inserted_visible =
        (at..at + added).all(|index| after.chars.get(index).is_some_and(|style| !style.hidden));
    let kept = markup.chars.iter().enumerate().all(|(index, style)| {
        let moved = if index < at { index } else { index + added };
        style.hidden || after.chars.get(moved) == Some(style)
    });
    (inserted_visible && kept).then_some(after)
}

/// Estilo que tendría un carácter escrito en la columna `col` de `line` (lo que se ve en
/// los botones de la barra sin selección).
pub fn typing_style(line: &str, col: usize) -> CharStyle {
    let mut chars: Vec<char> = line.chars().collect();
    let col = col.min(chars.len());
    chars.insert(col, 'x');
    let text: String = chars.into_iter().collect();
    parse_line(&text)
        .chars
        .get(col)
        .map(|style| CharStyle {
            hidden: false,
            bullet: false,
            checkbox: false,
            ..*style
        })
        .unwrap_or_default()
}

/// Columna donde escribir `text` (solo espacios o tabuladores) con el cursor en `col`, junto
/// a una marca oculta, sin romper el formato: `**hola **` ya no es negrita, así que un espacio
/// al final de una negrita va detrás de su `**` y uno al principio, delante. Se prueba cada
/// columna de la marca y se queda la primera (empezando por `col`) que deja igual el estilo de
/// todo lo visible. Si ninguna lo consigue, o no hay marca, `col`.
pub fn spacing_column(line: &str, col: usize, text: &str) -> usize {
    if text.is_empty() || !text.chars().all(|c| c == ' ' || c == '\t') {
        return col;
    }
    let markup = parse_line(line);
    if matches!(markup.block, Block::Raw | Block::Code | Block::Image) {
        return col;
    }
    let Some(run) = markup
        .hidden_runs()
        .into_iter()
        .find(|run| run.start <= col && col <= run.end)
    else {
        return col;
    };
    std::iter::once(col)
        .chain(run.start..=run.end)
        .filter(|at| *at >= markup.prefix)
        .find(|at| insertion_keeps_styles(line, &markup, *at, text).is_some())
        .unwrap_or(col)
}

/// Analiza una línea sabiendo si está dentro de un bloque de código ([`CodeFences`]):
/// dentro, todo se ve tal cual ([`Block::Code`]).
pub fn parse_line_in(line: &str, in_code: bool) -> LineMarkup {
    if in_code {
        return LineMarkup {
            block: Block::Code,
            chars: vec![CharStyle::default(); line.chars().count()],
            prefix: 0,
        };
    }
    parse_line(line)
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

    if let Some(image) = parse_image(line) {
        for (index, style) in styles.iter_mut().enumerate() {
            style.hidden = !image.alt_range.contains(&index);
        }
        return LineMarkup {
            block: Block::Image,
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
    } else if let Some(done) = task_state(chars.get(indent..).unwrap_or_default()) {
        // «- [ ] »: se ve solo la casilla (en el «[») y el espacio que la sigue.
        for (offset, style) in styles.iter_mut().skip(indent).take(5).enumerate() {
            if offset == 2 {
                style.checkbox = true;
            } else {
                style.hidden = true;
            }
        }
        (Block::Task(done), indent + 6)
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

    #[test]
    fn spaces_stay_outside_the_markers() {
        // Al final de la negrita (cursor delante del `**` de cierre): detrás.
        assert_eq!(spacing_column("**hola**", 6, " "), 8);
        // Al principio (cursor detrás del `**` de apertura): delante.
        assert_eq!(spacing_column("**hola**", 2, " "), 0);
        // Dentro del texto o sin marca al lado, donde está el cursor.
        assert_eq!(spacing_column("**hola**", 4, " "), 4);
        assert_eq!(spacing_column("hola", 4, " "), 4);
        // Las letras siguen dentro de la negrita.
        assert_eq!(spacing_column("**hola**", 6, "x"), 6);
        // Varias marcas juntas: entre la negrita que se cierra y la cursiva que empieza.
        assert_eq!(spacing_column("**a***b*", 3, " "), 5);
        assert_eq!(spacing_column("- ~~tachado~~", 11, " "), 13);
        assert_eq!(spacing_column("# **Título**", 10, " "), 12);
    }

    #[test]
    fn code_fences_mark_their_content() {
        let mut fences = CodeFences::default();
        let lines = [
            "texto",
            "```rust",
            "let **a** = 1;",
            "~~~",
            "```",
            "**b**",
            "~~~",
            "x",
        ];
        let inside: Vec<bool> = lines.iter().map(|l| fences.next_line(l)).collect();
        assert_eq!(
            inside,
            [false, false, true, true, false, false, false, true]
        );
        let markup = parse_line_in("let **a** = 1;", true);
        assert_eq!(markup.block, Block::Code);
        assert!(markup.chars.iter().all(|style| !style.hidden));
    }

    #[test]
    fn task_lines_show_a_checkbox() {
        let markup = parse_line("  - [x] pan");
        assert_eq!(markup.block, Block::Task(true));
        assert_eq!(markup.prefix, 8);
        let shown: String = "  - [x] pan"
            .chars()
            .zip(&markup.chars)
            .filter(|(_, style)| !style.hidden)
            .map(|(c, style)| if style.checkbox { '☑' } else { c })
            .collect();
        assert_eq!(shown, "  ☑ pan");
        assert_eq!(parse_line("- [ ] a").block, Block::Task(false));
        assert_eq!(parse_line("- [ ]a").block, Block::Bullet);
        assert_eq!(parse_line("- [y] a").block, Block::Bullet);
    }

    #[test]
    fn tasks_toggle_and_continue() {
        assert_eq!(toggle_task("- [ ] pan").as_deref(), Some("- [x] pan"));
        assert_eq!(toggle_task("  * [X] pan").as_deref(), Some("  * [ ] pan"));
        assert_eq!(toggle_task("- pan"), None);
        assert_eq!(
            continue_list("- [x] pan"),
            Some(ListContinuation::Continue("- [ ] ".to_owned()))
        );
        assert_eq!(continue_list("- [ ] "), Some(ListContinuation::End(6)));
    }

    #[test]
    fn detects_markdown_being_typed() {
        for typed in [
            "# ",
            "### ",
            "- ",
            "  * ",
            "1. ",
            "12) ",
            "hola **mundo**",
            "~~no~~",
        ] {
            assert!(typed_markdown(typed), "{typed:?}");
        }
        for typed in [
            "#",
            "####### ",
            "#a ",
            "hola ",
            "- a ",
            "1234. ",
            "a. ",
            "2 * 3 ",
            "**",
            "** a**",
            "**a **",
            "****",
            "a ~~ b ~~",
            "uno*",
        ] {
            assert!(!typed_markdown(typed), "{typed:?}");
        }
    }

    #[test]
    fn style_for_the_format_bar() {
        let markup = parse_line("# uno **dos** ~~tres~~");
        // Cursor tras «dos» (antes de las marcas de cierre o después): negrita.
        let bold = |range| markup.style_in(range).is_some_and(|style| style.bold);
        assert!(bold(11..11));
        assert!(bold(13..13));
        // Al principio del texto del título: el carácter siguiente.
        assert!(!bold(2..2));
        assert!(!bold(0..0));
        // Selección: todos los caracteres visibles.
        assert!(bold(6..13));
        assert!(!bold(2..13));
        assert!(markup.style_in(16..20).is_some_and(|style| style.strike));
        assert_eq!(markup.style_in(0..2), None);
        assert_eq!(parse_line("").style_in(0..0), None);
    }

    #[test]
    fn image_lines_only_show_their_description() {
        let markup = parse_line("![Un gato](gato.png)");
        assert_eq!(markup.block, Block::Image);
        let shown: String = "![Un gato](gato.png)"
            .chars()
            .zip(&markup.chars)
            .filter(|(_, style)| !style.hidden)
            .map(|(c, _)| c)
            .collect();
        assert_eq!(shown, "Un gato");
        // Con más texto en la línea no es una imagen.
        assert_ne!(parse_line("Mira ![x](a.png)").block, Block::Image);
    }

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
