//! Barras de desplazamiento del editor: flechas en los extremos, tamaño y posición del
//! tirador, y conversión de la posición del tirador en desplazamiento.

use std::time::Duration;

/// Grosor de las barras de desplazamiento (y lado de las flechas).
pub const SCROLLBAR_SIZE: f32 = 12.0;
/// Longitud mínima del tirador, para que se pueda agarrar en documentos muy largos.
const MIN_THUMB: f32 = 24.0;
/// Espera antes de repetir al mantener pulsada una flecha, y ritmo de la repetición.
pub const ARROW_DELAY: Duration = Duration::from_millis(350);
pub const ARROW_REPEAT: Duration = Duration::from_millis(40);

/// Eje de una barra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Vertical,
    Horizontal,
}

/// Parte de la barra bajo el puntero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Flecha del principio (arriba o izquierda).
    Back,
    /// Flecha del final (abajo o derecha).
    Forward,
    Thumb,
    /// Guía, fuera del tirador.
    Guide,
}

/// Barra de un eje, en coordenadas de pantalla a lo largo de ese eje.
///
/// `start`/`length` son la guía por la que corre el tirador, sin las flechas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Track {
    pub start: f32,
    pub length: f32,
    pub thumb_start: f32,
    pub thumb_length: f32,
    /// Lado de cada flecha (0 si la barra es tan corta que no caben).
    pub arrow: f32,
    max_scroll: f32,
}

impl Track {
    /// Barra que ocupa `length` desde `start` (flechas incluidas) para un contenido de
    /// `content` píxeles del que se ven `visible`. `None` si el contenido cabe y no hace
    /// falta barra.
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
        // Las flechas solo se ponen si dejan sitio para un tirador mínimo.
        let arrow = if length >= 2.0 * SCROLLBAR_SIZE + MIN_THUMB {
            SCROLLBAR_SIZE
        } else {
            0.0
        };
        let guide_start = start + arrow;
        let guide = length - 2.0 * arrow;
        let total = max_scroll + visible;
        let thumb_length = (guide * visible / total).clamp(MIN_THUMB.min(guide), guide);
        let progress = (scroll / max_scroll).clamp(0.0, 1.0);
        Some(Self {
            start: guide_start,
            length: guide,
            thumb_start: guide_start + progress * (guide - thumb_length),
            thumb_length,
            arrow,
            max_scroll,
        })
    }

    /// Qué parte de la barra hay en `position`.
    pub fn hit(&self, position: f32) -> Hit {
        if position < self.start {
            Hit::Back
        } else if position >= self.start + self.length {
            Hit::Forward
        } else if position >= self.thumb_start && position <= self.thumb_start + self.thumb_length {
            Hit::Thumb
        } else {
            Hit::Guide
        }
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

    /// Barra de 224 px desde 10: flechas de 10 a 22 y de 222 a 234, guía de 200 px.
    fn track(scroll: f32) -> Track {
        Track::new(10.0, 224.0, 100.0, 150.0, 100.0, scroll).unwrap()
    }

    #[test]
    fn no_bar_when_content_fits() {
        assert_eq!(Track::new(0.0, 100.0, 100.0, 80.0, 0.0, 0.0), None);
        assert_eq!(Track::new(0.0, 100.0, 100.0, 100.0, 50.0, 0.0), None);
    }

    #[test]
    fn arrows_take_both_ends() {
        let track = track(0.0);
        assert_eq!(track.arrow, SCROLLBAR_SIZE);
        assert_eq!((track.start, track.length), (22.0, 200.0));
        assert_eq!(track.hit(15.0), Hit::Back);
        assert_eq!(track.hit(230.0), Hit::Forward);
        assert_eq!(track.hit(50.0), Hit::Thumb);
        assert_eq!(track.hit(200.0), Hit::Guide);
        // Barra muy corta: sin flechas, toda es guía.
        let short = Track::new(0.0, 40.0, 100.0, 150.0, 100.0, 0.0).unwrap();
        assert_eq!(short.arrow, 0.0);
        assert_eq!(short.length, 40.0);
    }

    #[test]
    fn thumb_is_proportional_and_follows_scroll() {
        // Se ve la mitad del recorrido total: el tirador ocupa media guía.
        assert_eq!(track(0.0).thumb_length, 100.0);
        assert_eq!(track(0.0).thumb_start, 22.0);
        assert_eq!(track(100.0).thumb_start, 122.0);
    }

    #[test]
    fn thumb_has_a_minimum_length() {
        let track = Track::new(0.0, 224.0, 100.0, 1e6, 1e6, 0.0).unwrap();
        assert_eq!(track.thumb_length, MIN_THUMB);
    }

    #[test]
    fn thumb_position_maps_back_to_scroll() {
        let track = track(0.0);
        assert_eq!(track.scroll_at(22.0), 0.0);
        assert_eq!(track.scroll_at(72.0), 50.0);
        assert_eq!(track.scroll_at(122.0), 100.0);
        // Fuera de la guía se queda en los extremos.
        assert_eq!(track.scroll_at(-50.0), 0.0);
        assert_eq!(track.scroll_at(500.0), 100.0);
    }
}
