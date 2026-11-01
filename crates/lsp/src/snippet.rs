//! Textos de autocompletado en formato snippet (`foo(${1:x})$0`) convertidos en texto
//! plano y la posición donde dejar el cursor.

/// Texto ya expandido de un snippet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expanded {
    pub text: String,
    /// Posición (en caracteres desde el inicio de `text`) del primer hueco: `$1` si existe,
    /// si no `$0`. `None` si no hay huecos.
    pub cursor: Option<usize>,
}

/// Quita la sintaxis de snippet: los huecos se sustituyen por su texto por defecto (o
/// por nada), las opciones `${1|a,b|}` por la primera y las variables por nada.
pub fn expand(snippet: &str) -> Expanded {
    let chars: Vec<char> = snippet.chars().collect();
    let mut parser = Parser {
        chars: &chars,
        index: 0,
        out: String::new(),
        out_len: 0,
        stops: Vec::new(),
    };
    parser.run(None);
    let first = parser
        .stops
        .iter()
        .filter(|(n, _)| *n > 0)
        .min_by_key(|(n, _)| *n)
        .or_else(|| parser.stops.iter().find(|(n, _)| *n == 0))
        .map(|(_, at)| *at);
    Expanded {
        text: parser.out,
        cursor: first,
    }
}

struct Parser<'a> {
    chars: &'a [char],
    index: usize,
    out: String,
    /// Longitud de `out` en caracteres.
    out_len: usize,
    /// (número de hueco, posición en `out`).
    stops: Vec<(u32, usize)>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn push(&mut self, c: char) {
        self.out.push(c);
        self.out_len += 1;
    }

    fn number(&mut self) -> Option<u32> {
        let start = self.index;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.index += 1;
        }
        let digits: String = self.chars.get(start..self.index)?.iter().collect();
        digits.parse().ok()
    }

    fn name(&mut self) -> bool {
        let start = self.index;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            self.index += 1;
        }
        self.index > start
    }

    /// Copia texto hasta `}` (si `until` lo indica) o hasta el final.
    fn run(&mut self, until: Option<char>) {
        while let Some(c) = self.peek() {
            if Some(c) == until {
                return;
            }
            self.index += 1;
            match c {
                '\\' => match self.peek() {
                    Some(next @ ('$' | '}' | '\\' | ',' | '|')) => {
                        self.index += 1;
                        self.push(next);
                    }
                    _ => self.push('\\'),
                },
                '$' => self.dollar(),
                _ => self.push(c),
            }
        }
    }

    fn dollar(&mut self) {
        match self.peek() {
            Some(c) if c.is_ascii_digit() => {
                if let Some(n) = self.number() {
                    self.stops.push((n, self.out_len));
                }
            }
            Some('{') => {
                self.index += 1;
                if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    let n = self.number().unwrap_or(0);
                    self.stops.push((n, self.out_len));
                    match self.peek() {
                        Some(':') => {
                            self.index += 1;
                            self.run(Some('}'));
                        }
                        Some('|') => {
                            self.index += 1;
                            self.choice();
                        }
                        _ => {}
                    }
                } else if self.name() {
                    // Variable: `${TM_FILENAME}` o `${TM_FILENAME:defecto}`.
                    if self.peek() == Some(':') {
                        self.index += 1;
                        self.run(Some('}'));
                    } else {
                        self.skip_to_brace();
                    }
                } else {
                    self.push('$');
                    self.push('{');
                    return;
                }
                if self.peek() == Some('}') {
                    self.index += 1;
                }
            }
            Some(c) if c.is_ascii_alphabetic() || c == '_' => {
                self.name();
            }
            _ => self.push('$'),
        }
    }

    /// `${1|uno,dos|}`: se queda con la primera opción.
    fn choice(&mut self) {
        let mut first = true;
        while let Some(c) = self.peek() {
            self.index += 1;
            match c {
                '|' => return,
                ',' => first = false,
                '\\' => {
                    if let Some(next) = self.peek() {
                        self.index += 1;
                        if first {
                            self.push(next);
                        }
                    }
                }
                _ if first => self.push(c),
                _ => {}
            }
        }
    }

    fn skip_to_brace(&mut self) {
        while self.peek().is_some_and(|c| c != '}') {
            self.index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> String {
        expand(s).text
    }

    #[test]
    fn plain_text_is_unchanged() {
        assert_eq!(
            expand("verde"),
            Expanded {
                text: "verde".into(),
                cursor: None
            }
        );
    }

    #[test]
    fn tabstops_and_placeholders() {
        let e = expand("println!(\"${1:texto}\")$0");
        assert_eq!(e.text, "println!(\"texto\")");
        assert_eq!(e.cursor, Some(10));
        let e = expand("fn ${1:nombre}(${2}) {\n\t$0\n}");
        assert_eq!(e.text, "fn nombre() {\n\t\n}");
        assert_eq!(e.cursor, Some(3));
        assert_eq!(expand("foo($0)").cursor, Some(4));
    }

    #[test]
    fn nested_choices_variables_and_escapes() {
        assert_eq!(text("${1:a ${2:b}} c"), "a b c");
        assert_eq!(text("${1|uno,dos|}"), "uno");
        assert_eq!(text("$TM_FILENAME-${TM_LINE:7}"), "-7");
        assert_eq!(text("cuesta \\$5 \\}"), "cuesta $5 }");
        assert_eq!(text("solo $"), "solo $");
        assert_eq!(text("ñ${1:á}🌙"), "ñá🌙");
        assert_eq!(expand("ñ🌙$1").cursor, Some(2));
    }
}
