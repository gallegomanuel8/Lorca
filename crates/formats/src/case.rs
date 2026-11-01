//! Conversión entre mayúsculas y minúsculas.

/// Tipo de conversión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Case {
    Upper,
    Lower,
    /// Primera letra de cada palabra en mayúscula y el resto en minúscula.
    Title,
}

/// Convierte el texto. Respeta las reglas de Unicode (la ß pasa a «SS», etc.).
pub fn change_case(text: &str, case: Case) -> String {
    match case {
        Case::Upper => text.to_uppercase(),
        Case::Lower => text.to_lowercase(),
        Case::Title => {
            let mut out = String::with_capacity(text.len());
            let mut at_word_start = true;
            for c in text.chars() {
                if c.is_alphanumeric() {
                    if at_word_start {
                        out.extend(c.to_uppercase());
                    } else {
                        out.extend(c.to_lowercase());
                    }
                    at_word_start = false;
                } else {
                    out.push(c);
                    at_word_start = c != '\'' && c != '’';
                }
            }
            out
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversions() {
        assert_eq!(change_case("Ñandú", Case::Upper), "ÑANDÚ");
        assert_eq!(change_case("ÁRBOL", Case::Lower), "árbol");
        assert_eq!(
            change_case("bodas de SANGRE", Case::Title),
            "Bodas De Sangre"
        );
        assert_eq!(change_case("l'amour", Case::Title), "L'amour");
    }
}
