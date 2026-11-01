//! Marco de los mensajes JSON-RPC: cabecera `Content-Length` y cuerpo JSON.

use std::io::{BufRead, Write};

use serde_json::Value;

use crate::LspError;

/// Escribe un mensaje con su cabecera.
pub fn write_message(writer: &mut impl Write, message: &Value) -> Result<(), LspError> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

/// Lee el siguiente mensaje. `None` si el servidor cerró la salida.
pub fn read_message(reader: &mut impl BufRead) -> Result<Option<Value>, LspError> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            if length.is_some() {
                break;
            }
            // Líneas en blanco sueltas entre mensajes: se ignoran.
            continue;
        }
        let Some((name, value)) = header.split_once(':') else {
            return Err(LspError::Protocol(format!("cabecera inválida «{header}»")));
        };
        if name.trim().eq_ignore_ascii_case("content-length") {
            let parsed = value
                .trim()
                .parse::<usize>()
                .map_err(|_| LspError::Protocol(format!("longitud inválida «{value}»")))?;
            length = Some(parsed);
        }
    }
    let length = length.unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trip_with_unicode() {
        let mut buffer = Vec::new();
        let first = json!({"jsonrpc": "2.0", "method": "a", "params": {"texto": "verde ñ 🌙"}});
        let second = json!({"jsonrpc": "2.0", "id": 1, "result": null});
        write_message(&mut buffer, &first).unwrap();
        write_message(&mut buffer, &second).unwrap();
        let mut reader = std::io::Cursor::new(buffer);
        assert_eq!(read_message(&mut reader).unwrap(), Some(first));
        assert_eq!(read_message(&mut reader).unwrap(), Some(second));
        assert_eq!(read_message(&mut reader).unwrap(), None);
    }

    #[test]
    fn extra_headers_are_ignored() {
        let body = r#"{"id":1}"#;
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc; charset=utf-8\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        );
        let mut reader = std::io::Cursor::new(raw.into_bytes());
        assert_eq!(read_message(&mut reader).unwrap(), Some(json!({"id": 1})));
    }

    #[test]
    fn bad_header_is_an_error() {
        let mut reader = std::io::Cursor::new(b"basura\r\n\r\n".to_vec());
        assert!(matches!(
            read_message(&mut reader),
            Err(LspError::Protocol(_))
        ));
    }
}
