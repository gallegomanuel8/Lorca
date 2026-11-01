//! Conversión entre los bytes de un archivo y el texto normalizado del buffer.

use crate::{CoreError, LineEnding, TextEncoding, decode, encode};

/// Formato de un archivo en disco: codificación y final de línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextFormat {
    pub encoding: TextEncoding,
    pub line_ending: LineEnding,
}

impl Default for TextFormat {
    /// Formato para documentos nuevos: UTF-8 y el final de línea de la plataforma.
    fn default() -> Self {
        Self {
            encoding: TextEncoding::Utf8,
            line_ending: LineEnding::platform_default(),
        }
    }
}

/// Texto cargado desde bytes, ya normalizado a LF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedText {
    pub text: String,
    pub format: TextFormat,
    /// `true` si hubo bytes inválidos sustituidos durante la decodificación.
    pub had_decoding_errors: bool,
}

/// Decodifica los bytes de un archivo y normaliza los finales de línea a LF.
pub fn load_bytes(bytes: &[u8]) -> LoadedText {
    let decoded = decode(bytes);
    let line_ending =
        LineEnding::detect(&decoded.text).unwrap_or_else(LineEnding::platform_default);
    let text = LineEnding::normalize(&decoded.text).into_owned();
    LoadedText {
        text,
        format: TextFormat {
            encoding: decoded.encoding,
            line_ending,
        },
        had_decoding_errors: decoded.had_errors,
    }
}

/// Convierte el texto normalizado del buffer a los bytes que se escriben en disco.
pub fn save_bytes(text: &str, format: TextFormat) -> Result<Vec<u8>, CoreError> {
    encode(&format.line_ending.apply(text), format.encoding)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_file_round_trips() {
        let original = "línea 1\r\nlínea 2\r\n".as_bytes();
        let loaded = load_bytes(original);
        assert_eq!(loaded.text, "línea 1\nlínea 2\n");
        assert_eq!(loaded.format.line_ending, LineEnding::CrLf);
        assert_eq!(loaded.format.encoding, TextEncoding::Utf8);
        assert_eq!(save_bytes(&loaded.text, loaded.format).unwrap(), original);
    }

    #[test]
    fn file_without_newlines_uses_platform_default() {
        let loaded = load_bytes(b"hola");
        assert_eq!(loaded.format.line_ending, LineEnding::platform_default());
    }
}
