//! Posiciones lógicas (línea y columna) dentro de un buffer.

/// Posición lógica en el texto. Ambos valores empiezan en cero.
///
/// La columna se mide en caracteres Unicode (no en bytes ni en grafemas).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}
