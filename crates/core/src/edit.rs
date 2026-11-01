//! Operaciones atómicas de edición.

use crate::{Buffer, CoreError};

/// Operación elemental sobre el buffer. Guarda el texto afectado para poder invertirla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// Inserta `text` en el índice `at`.
    Insert { at: usize, text: String },
    /// Elimina `text`, que empieza en el índice `at`.
    Delete { at: usize, text: String },
}

impl Edit {
    /// Operación que deshace esta.
    pub fn inverse(&self) -> Self {
        match self {
            Self::Insert { at, text } => Self::Delete {
                at: *at,
                text: text.clone(),
            },
            Self::Delete { at, text } => Self::Insert {
                at: *at,
                text: text.clone(),
            },
        }
    }

    /// Aplica la operación al buffer.
    pub fn apply(&self, buffer: &mut Buffer) -> Result<(), CoreError> {
        match self {
            Self::Insert { at, text } => buffer.insert(*at, text),
            Self::Delete { at, text } => buffer.remove(*at..*at + text.chars().count()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_then_inverse_restores_text() {
        let mut b = Buffer::from_text("la luna");
        let edits = [
            Edit::Insert {
                at: 7,
                text: " vino".into(),
            },
            Edit::Delete {
                at: 0,
                text: "la ".into(),
            },
        ];
        for e in &edits {
            e.apply(&mut b).unwrap();
        }
        assert_eq!(b.to_string(), "luna vino");
        for e in edits.iter().rev() {
            e.inverse().apply(&mut b).unwrap();
        }
        assert_eq!(b.to_string(), "la luna");
    }

    #[test]
    fn delete_counts_chars_not_bytes() {
        let mut b = Buffer::from_text("añoñ");
        Edit::Delete {
            at: 1,
            text: "ño".into(),
        }
        .apply(&mut b)
        .unwrap();
        assert_eq!(b.to_string(), "añ");
    }
}
