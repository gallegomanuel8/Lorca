//! Detección y conversión de codificaciones de texto.

use encoding_rs::{Encoding, UTF_8, UTF_16BE, WINDOWS_1252};

use crate::CoreError;

/// Codificaciones soportadas al leer y escribir archivos.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TextEncoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl TextEncoding {
    /// Nombre para mostrar en la interfaz.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 con BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Windows1252 => "Windows-1252",
        }
    }
}

/// Resultado de decodificar bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded {
    pub text: String,
    pub encoding: TextEncoding,
    /// `true` si hubo secuencias inválidas sustituidas por U+FFFD.
    pub had_errors: bool,
}

/// Decodifica bytes detectando la codificación.
///
/// Orden: BOM (UTF-8, UTF-16 LE/BE) → UTF-8 válido → Windows-1252 como último recurso
/// (nunca falla, porque asigna un carácter a cada byte).
pub fn decode(bytes: &[u8]) -> Decoded {
    if let Some((bom_encoding, bom_len)) = Encoding::for_bom(bytes) {
        let encoding = if bom_encoding == UTF_8 {
            TextEncoding::Utf8Bom
        } else if bom_encoding == UTF_16BE {
            TextEncoding::Utf16Be
        } else {
            TextEncoding::Utf16Le
        };
        let body = bytes.get(bom_len..).unwrap_or_default();
        let (text, had_errors) = bom_encoding.decode_without_bom_handling(body);
        return Decoded {
            text: text.into_owned(),
            encoding,
            had_errors,
        };
    }

    if let Ok(text) = std::str::from_utf8(bytes) {
        return Decoded {
            text: text.to_owned(),
            encoding: TextEncoding::Utf8,
            had_errors: false,
        };
    }

    let (text, had_errors) = WINDOWS_1252.decode_without_bom_handling(bytes);
    Decoded {
        text: text.into_owned(),
        encoding: TextEncoding::Windows1252,
        had_errors,
    }
}

/// Codifica texto en la codificación indicada (incluyendo BOM cuando corresponde).
pub fn encode(text: &str, encoding: TextEncoding) -> Result<Vec<u8>, CoreError> {
    let bytes = match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Utf8Bom => {
            let mut out = Vec::with_capacity(text.len() + 3);
            out.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
            out.extend_from_slice(text.as_bytes());
            out
        }
        // encoding_rs no codifica a UTF-16 (sigue el estándar WHATWG), así que se hace a mano.
        TextEncoding::Utf16Le => {
            let mut out = vec![0xFF, 0xFE];
            out.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
            out
        }
        TextEncoding::Utf16Be => {
            let mut out = vec![0xFE, 0xFF];
            out.extend(text.encode_utf16().flat_map(u16::to_be_bytes));
            out
        }
        TextEncoding::Windows1252 => {
            let (bytes, _, had_errors) = WINDOWS_1252.encode(text);
            if had_errors {
                return Err(CoreError::Unencodable {
                    encoding: encoding.label(),
                });
            }
            bytes.into_owned()
        }
    };
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_utf8() {
        let d = decode("canción".as_bytes());
        assert_eq!(d.encoding, TextEncoding::Utf8);
        assert_eq!(d.text, "canción");
        assert!(!d.had_errors);
    }

    #[test]
    fn empty_input_is_utf8() {
        assert_eq!(decode(b"").encoding, TextEncoding::Utf8);
    }

    #[test]
    fn utf8_with_bom() {
        let d = decode(&[0xEF, 0xBB, 0xBF, b'h', b'i']);
        assert_eq!(d.encoding, TextEncoding::Utf8Bom);
        assert_eq!(d.text, "hi");
    }

    #[test]
    fn utf16_round_trip() {
        for enc in [TextEncoding::Utf16Le, TextEncoding::Utf16Be] {
            let bytes = encode("verde que te quiero 🌿", enc).unwrap();
            let d = decode(&bytes);
            assert_eq!(d.encoding, enc);
            assert_eq!(d.text, "verde que te quiero 🌿");
        }
    }

    #[test]
    fn invalid_utf8_falls_back_to_windows_1252() {
        // "canción" en Windows-1252: la ó es 0xF3.
        let d = decode(&[b'c', b'a', b'n', b'c', b'i', 0xF3, b'n']);
        assert_eq!(d.encoding, TextEncoding::Windows1252);
        assert_eq!(d.text, "canción");
        assert_eq!(
            encode(&d.text, TextEncoding::Windows1252).unwrap(),
            b"canci\xF3n"
        );
    }

    #[test]
    fn windows_1252_rejects_unrepresentable() {
        assert_eq!(
            encode("日本", TextEncoding::Windows1252),
            Err(CoreError::Unencodable {
                encoding: "Windows-1252"
            })
        );
    }

    #[test]
    fn utf8_bom_is_written() {
        assert_eq!(
            encode("a", TextEncoding::Utf8Bom).unwrap(),
            vec![0xEF, 0xBB, 0xBF, b'a']
        );
    }
}
