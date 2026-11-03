//! Barras de desplazamiento del editor: tamaño y posición del tirador, y conversión de la
//! posición del tirador en desplazamiento.

/// Grosor de las barras de desplazamiento.
pub const SCROLLBAR_SIZE: f32 = 12.0;
/// Longitud mínima del tirador, para que se pueda agarrar en documentos muy largos.
const MIN_THUMB: f32 = 24.0;

/// Eje de una barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Barra de un eje, en coordenadas de pantalla a lo largo de ese eje.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    pub start: f32,
    pub length: f32,
    pub thumb_start: f32,
    pub thumb_length: f32,
    max_scroll: f32,
}

impl Track {
    /// Barra que ocupa `length` desde `start` para un contenido de `content` píxeles del
    /// que se ven `visible`. `None` si el contenido cabe y no hace falta barra.
    pub fn new(
        start: f32,
        length: f32,
        visible: f32,
        content: f32,
        max_scroll: f32,
        scroll: f32,
    ) -> Option<Self> {
        if content <= visible || max_scroll <= 0.0 || length <= 0.0 {
            return None;
        }
        let total = max_scroll + visible;
        let thumb_length = (length * visible / total).clamp(MIN_THUMB.min(length), length);
        let progress = (scroll / max_scroll).clamp(0.0, 1.0);
        Some(Self {
            start,
            length,
            thumb_start: start + progress * (length - thumb_length),
            thumb_length,
            max_scroll,
        })
    }

    /// `true` si `position` cae sobre el tirador.
    pub fn is_on_thumb(&self, position: f32) -> bool {
        position >= self.thumb_start && position <= self.thumb_start + self.thumb_length
    }

    /// Desplazamiento que corresponde a tener el tirador empezando en `thumb_start`.
    pub fn scroll_at(&self, thumb_start: f32) -> f32 {
        let free = self.length - self.thumb_length;
        if free <= 0.0 {
            return 0.0;
        }
        ((thumb_start - self.start) / free).clamp(0.0, 1.0) * self.max_scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_bar_when_content_fits() {
        assert_eq!(Track::new(0.0, 100.0, 100.0, 80.0, 0.0, 0.0), None);
        assert_eq!(Track::new(0.0, 100.0, 100.0, 100.0, 50.0, 0.0), None);
    }

    #[test]
    fn thumb_is_proportional_and_follows_scroll() {
        // Se ve la mitad del recorrido total: el tirador ocupa media guía.
        let top = Track::new(10.0, 200.0, 100.0, 150.0, 100.0, 0.0).unwrap();
        assert_eq!(top.thumb_length, 100.0);
        assert_eq!(top.thumb_start, 10.0);
        let bottom = Track::new(10.0, 200.0, 100.0, 150.0, 100.0, 100.0).unwrap();
        assert_eq!(bottom.thumb_start, 110.0);
    }

    #[test]
    fn thumb_has_a_minimum_length() {
        let track = Track::new(0.0, 200.0, 100.0, 1e6, 1e6, 0.0).unwrap();
        assert_eq!(track.thumb_length, MIN_THUMB);
    }

    #[test]
    fn thumb_position_maps_back_to_scroll() {
        let track = Track::new(10.0, 200.0, 100.0, 150.0, 100.0, 0.0).unwrap();
        assert_eq!(track.scroll_at(10.0), 0.0);
        assert_eq!(track.scroll_at(60.0), 50.0);
        assert_eq!(track.scroll_at(110.0), 100.0);
        // Fuera de la guía se queda en los extremos.
        assert_eq!(track.scroll_at(-50.0), 0.0);
        assert_eq!(track.scroll_at(500.0), 100.0);
        assert!(track.is_on_thumb(50.0));
        assert!(!track.is_on_thumb(150.0));
    }
}
