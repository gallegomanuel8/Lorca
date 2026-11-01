//! Conversión de columnas entre el editor (caracteres) y el servidor (UTF-8, UTF-16 o
//! UTF-32, según se negocie al iniciar).

use lsp_types::PositionEncodingKind;

/// Unidad en que el servidor cuenta las columnas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PositionEncoding {
    Utf8,
    /// La predeterminada del protocolo si el servidor no dice otra.
    #[default]
    Utf16,
    /// Una unidad por carácter: coincide con las columnas del editor.
    Utf32,
}

impl PositionEncoding {
    pub fn from_kind(kind: &PositionEncodingKind) -> Self {
        match kind.as_str() {
            "utf-8" => Self::Utf8,
            "utf-32" => Self::Utf32,
            _ => Self::Utf16,
        }
    }

    fn units(self, c: char) -> usize {
        match self {
            Self::Utf8 => c.len_utf8(),
            Self::Utf16 => c.len_utf16(),
            Self::Utf32 => 1,
        }
    }

    /// Columna del servidor para la columna `char_column` (en caracteres) de `line`.
    pub fn to_server(self, line: &str, char_column: usize) -> u32 {
        let units: usize = line.chars().take(char_column).map(|c| self.units(c)).sum();
        u32::try_from(units).unwrap_or(u32::MAX)
    }

    /// Columna en caracteres para la columna `column` del servidor. Se limita al final de
    /// la línea; una columna en mitad de un carácter cuenta como ese carácter.
    pub fn to_chars(self, line: &str, column: u32) -> usize {
        let target = usize::try_from(column).unwrap_or(usize::MAX);
        let mut units = 0;
        for (index, c) in line.chars().enumerate() {
            if units >= target {
                return index;
            }
            units += self.units(c);
        }
        line.chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINE: &str = "a🌙ñb";

    #[test]
    fn utf16_counts_surrogate_pairs() {
        let e = PositionEncoding::Utf16;
        assert_eq!(e.to_server(LINE, 2), 3);
        assert_eq!(e.to_chars(LINE, 3), 2);
        assert_eq!(e.to_server(LINE, 4), 5);
        // Más allá del final: el final.
        assert_eq!(e.to_chars(LINE, 99), 4);
    }

    #[test]
    fn utf8_counts_bytes() {
        let e = PositionEncoding::Utf8;
        assert_eq!(e.to_server(LINE, 3), 1 + 4 + 2);
        assert_eq!(e.to_chars(LINE, 7), 3);
    }

    #[test]
    fn utf32_is_identity() {
        let e = PositionEncoding::Utf32;
        assert_eq!(e.to_server(LINE, 3), 3);
        assert_eq!(e.to_chars(LINE, 3), 3);
    }

    #[test]
    fn negotiated_kind() {
        assert_eq!(
            PositionEncoding::from_kind(&PositionEncodingKind::UTF32),
            PositionEncoding::Utf32
        );
        assert_eq!(
            PositionEncoding::from_kind(&PositionEncodingKind::new("otra")),
            PositionEncoding::Utf16
        );
    }
}
