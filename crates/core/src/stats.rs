//! Recuento de palabras y caracteres (barra de estado de los textos).

use std::ops::Range;

use crate::Buffer;

/// Palabras y caracteres de un texto.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStats {
    /// Tramos sin espacios que tienen alguna letra o cifra (así no cuentan las marcas
    /// sueltas de Markdown como `#`, `-` o `**`).
    pub words: usize,
    /// Caracteres sin contar los saltos de línea.
    pub chars: usize,
}

impl TextStats {
    /// Cuenta los caracteres de `chars`.
    pub fn of(chars: impl IntoIterator<Item = char>) -> Self {
        let mut stats = Self::default();
        let mut in_word = false;
        let mut word_has_text = false;
        for c in chars {
            if c != '\n' {
                stats.chars += 1;
            }
            if c.is_whitespace() {
                if in_word && word_has_text {
                    stats.words += 1;
                }
                in_word = false;
                word_has_text = false;
            } else {
                in_word = true;
                word_has_text |= c.is_alphanumeric();
            }
        }
        if in_word && word_has_text {
            stats.words += 1;
        }
        stats
    }
}

impl Buffer {
    /// Palabras y caracteres de `range` (índices de carácter; se ajusta al texto).
    pub fn stats(&self, range: Range<usize>) -> TextStats {
        let len = self.len_chars();
        let start = range.start.min(len);
        let end = range.end.clamp(start, len);
        TextStats::of(self.rope().slice(start..end).chars())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_characters() {
        let stats = TextStats::of("# Verde que\n\n- te **quiero** verde.".chars());
        assert_eq!(stats.words, 5);
        assert_eq!(stats.chars, 33);
        assert_eq!(TextStats::of("".chars()), TextStats::default());
        assert_eq!(TextStats::of("  año 2024  ".chars()).words, 2);
    }

    #[test]
    fn buffer_ranges() {
        let buffer = Buffer::from_text("uno dos\ntres");
        assert_eq!(buffer.stats(0..usize::MAX).words, 3);
        assert_eq!(buffer.stats(4..10).words, 2);
        assert_eq!(buffer.stats(4..10).chars, 5);
    }
}
