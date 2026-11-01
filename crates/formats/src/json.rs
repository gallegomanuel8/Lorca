//! Formateo y minificación de JSON conservando el orden de las claves.

use serde::Serialize;
use serde_json::Value;
use serde_json::ser::{PrettyFormatter, Serializer};

/// Error al formatear: el texto no es JSON válido.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("JSON no válido en la línea {line}, columna {column}: {message}")]
pub struct FormatError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

fn parse(text: &str) -> Result<Value, FormatError> {
    serde_json::from_str(text).map_err(|error| FormatError {
        line: error.line(),
        column: error.column(),
        message: error.to_string(),
    })
}

/// Formatea con la sangría indicada (p. ej. cuatro espacios). Si el texto original
/// terminaba en salto de línea, el resultado también.
pub fn format_json(text: &str, indent: &str) -> Result<String, FormatError> {
    let value = parse(text)?;
    let mut out = Vec::new();
    let mut serializer =
        Serializer::with_formatter(&mut out, PrettyFormatter::with_indent(indent.as_bytes()));
    value
        .serialize(&mut serializer)
        .map_err(|error| FormatError {
            line: 0,
            column: 0,
            message: error.to_string(),
        })?;
    let mut formatted = String::from_utf8_lossy(&out).into_owned();
    if text.ends_with('\n') {
        formatted.push('\n');
    }
    Ok(formatted)
}

/// JSON en una sola línea, sin espacios.
pub fn minify_json(text: &str) -> Result<String, FormatError> {
    let value = parse(text)?;
    Ok(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_keeps_key_order() {
        let formatted = format_json(r#"{"z":1,"a":[true,null],"m":{"x":"ñ"}}"#, "    ").unwrap();
        assert_eq!(
            formatted,
            "{\n    \"z\": 1,\n    \"a\": [\n        true,\n        null\n    ],\n    \"m\": {\n        \"x\": \"ñ\"\n    }\n}"
        );
    }

    #[test]
    fn trailing_newline_is_kept() {
        assert_eq!(format_json("[1]\n", "  ").unwrap(), "[\n  1\n]\n");
    }

    #[test]
    fn minify() {
        assert_eq!(
            minify_json("{\n  \"a\": [1, 2]\n}").unwrap(),
            r#"{"a":[1,2]}"#
        );
    }

    #[test]
    fn invalid_json_reports_position() {
        let error = format_json("{\n  \"a\": ,\n}", "  ").unwrap_err();
        assert_eq!(error.line, 2);
        assert!(error.column > 0);
    }
}
