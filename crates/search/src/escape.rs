//! Secuencias de escape del modo extendido y de las plantillas de sustitución.

/// Interpreta `\n`, `\r`, `\t`, `\0`, `\\`, `\xHH` y `\uHHHH`. Cualquier otra secuencia
/// se deja tal cual (barra incluida).
pub fn unescape_extended(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some('n') => push_and_skip(&mut out, &mut chars, '\n'),
            Some('r') => push_and_skip(&mut out, &mut chars, '\r'),
            Some('t') => push_and_skip(&mut out, &mut chars, '\t'),
            Some('0') => push_and_skip(&mut out, &mut chars, '\0'),
            Some('\\') => push_and_skip(&mut out, &mut chars, '\\'),
            Some(kind @ ('x' | 'u')) => {
                let digits = if kind == 'x' { 2 } else { 4 };
                let hex: String = chars.clone().skip(1).take(digits).collect();
                let decoded = (hex.len() == digits)
                    .then(|| u32::from_str_radix(&hex, 16).ok())
                    .flatten()
                    .and_then(char::from_u32);
                match decoded {
                    Some(decoded) => {
                        out.push(decoded);
                        for _ in 0..=digits {
                            chars.next();
                        }
                    }
                    None => out.push('\\'),
                }
            }
            _ => out.push('\\'),
        }
    }
    out
}

fn push_and_skip(out: &mut String, chars: &mut impl Iterator<Item = char>, c: char) {
    out.push(c);
    chars.next();
}

/// Convierte una plantilla de sustitución estilo Notepad++ a la sintaxis de `regex`:
/// `\1` → `${1}`, `\n`/`\t`/`\r` → el carácter, `\\` → `\`. `$1` y `${nombre}` ya son
/// válidos y se dejan igual.
pub(crate) fn regex_template(replacement: &str) -> String {
    let mut out = String::with_capacity(replacement.len());
    let mut chars = replacement.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.peek().copied() {
            Some(digit @ '0'..='9') => {
                out.push_str("${");
                out.push(digit);
                out.push('}');
                chars.next();
            }
            Some('n') => push_and_skip(&mut out, &mut chars, '\n'),
            Some('r') => push_and_skip(&mut out, &mut chars, '\r'),
            Some('t') => push_and_skip(&mut out, &mut chars, '\t'),
            Some('\\') => push_and_skip(&mut out, &mut chars, '\\'),
            _ => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_sequences() {
        assert_eq!(unescape_extended(r"a\nb\tc\\d"), "a\nb\tc\\d");
        assert_eq!(unescape_extended(r"\x41ñ"), "Añ");
        assert_eq!(unescape_extended(r"\q\x4"), r"\q\x4");
        assert_eq!(unescape_extended(r"fin\"), r"fin\");
    }

    #[test]
    fn template_conversion() {
        assert_eq!(regex_template(r"\2-\1"), "${2}-${1}");
        assert_eq!(regex_template(r"$1\n${nombre}"), "$1\n${nombre}");
        assert_eq!(regex_template(r"a\\b\q"), r"a\b\q");
    }
}
