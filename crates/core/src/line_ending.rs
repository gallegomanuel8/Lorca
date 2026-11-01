//! Finales de línea: detección, normalización a LF y restauración al guardar.

use std::borrow::Cow;

/// Final de línea con el que se guarda un documento.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
}

impl LineEnding {
    /// Final de línea habitual en la plataforma actual.
    pub const fn platform_default() -> Self {
        if cfg!(windows) { Self::CrLf } else { Self::Lf }
    }

    /// Nombre corto para mostrar en la interfaz.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
        }
    }

    /// Identificador estable para guardar en archivos de sesión o configuración.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::CrLf => "crlf",
        }
    }

    /// Inverso de [`LineEnding::id`].
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "lf" => Some(Self::Lf),
            "crlf" => Some(Self::CrLf),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// Detecta el final de línea predominante. Devuelve `None` si el texto no tiene saltos.
    ///
    /// Un CR aislado (Mac clásico) se cuenta como LF, porque se normaliza igual.
    pub fn detect(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let (mut lf, mut crlf) = (0usize, 0usize);
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => {
                    crlf += 1;
                    i += 1;
                }
                b'\r' | b'\n' => lf += 1,
                _ => {}
            }
            i += 1;
        }
        match (lf, crlf) {
            (0, 0) => None,
            (lf, crlf) if crlf > lf => Some(Self::CrLf),
            _ => Some(Self::Lf),
        }
    }

    /// Convierte CRLF y CR aislados en LF.
    pub fn normalize(text: &str) -> Cow<'_, str> {
        if !text.contains('\r') {
            return Cow::Borrowed(text);
        }
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    }

    /// Convierte un texto normalizado (solo LF) a este final de línea.
    pub fn apply(self, text: &str) -> Cow<'_, str> {
        match self {
            Self::Lf => Cow::Borrowed(text),
            Self::CrLf => Cow::Owned(text.replace('\n', "\r\n")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_variants() {
        assert_eq!(LineEnding::detect("sin saltos"), None);
        assert_eq!(LineEnding::detect("a\nb\n"), Some(LineEnding::Lf));
        assert_eq!(LineEnding::detect("a\r\nb\r\n"), Some(LineEnding::CrLf));
        assert_eq!(LineEnding::detect("a\r\nb\r\nc\n"), Some(LineEnding::CrLf));
        assert_eq!(LineEnding::detect("a\r\nb\nc\n"), Some(LineEnding::Lf));
        assert_eq!(LineEnding::detect("a\rb"), Some(LineEnding::Lf));
    }

    #[test]
    fn ids_round_trip() {
        for ending in [LineEnding::Lf, LineEnding::CrLf] {
            assert_eq!(LineEnding::from_id(ending.id()), Some(ending));
        }
        assert_eq!(LineEnding::from_id("cr"), None);
    }

    #[test]
    fn normalize_converts_everything_to_lf() {
        assert_eq!(LineEnding::normalize("a\r\nb\rc\n"), "a\nb\nc\n");
        assert!(matches!(LineEnding::normalize("a\nb"), Cow::Borrowed(_)));
    }

    #[test]
    fn apply_round_trips() {
        let original = "uno\r\ndos\r\n";
        let normalized = LineEnding::normalize(original);
        assert_eq!(LineEnding::CrLf.apply(&normalized), original);
        assert_eq!(LineEnding::Lf.apply(&normalized), "uno\ndos\n");
    }
}
