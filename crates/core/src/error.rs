//! Errores del núcleo de edición.

/// Errores que pueden producirse al operar sobre el buffer o al codificar texto.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CoreError {
    /// Un índice de carácter queda fuera del buffer.
    #[error("índice {index} fuera de rango (longitud {len})")]
    OutOfBounds { index: usize, len: usize },

    /// Un rango tiene el inicio después del final o excede el buffer.
    #[error("rango inválido {start}..{end} (longitud {len})")]
    InvalidRange {
        start: usize,
        end: usize,
        len: usize,
    },

    /// El texto contiene caracteres que la codificación de destino no puede representar.
    #[error("el texto contiene caracteres que no se pueden representar en {encoding}")]
    Unencodable { encoding: &'static str },
}
