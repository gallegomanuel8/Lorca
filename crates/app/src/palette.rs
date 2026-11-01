//! Paleta de órdenes (⇧⌘P): búsqueda difusa sobre todas las acciones.

use iced::widget::{
    Id, button, column, container, mouse_area, opaque, row, space, stack, text, text_input,
};
use iced::{Alignment, Element, Length, Shadow, Theme, Vector};

use crate::app::Message;
use crate::key_filter::filter_command_keys;

pub const INPUT_ID: &str = "command-palette";
/// Máximo de órdenes que se muestran a la vez.
const MAX_VISIBLE: usize = 12;
const WIDTH: f32 = 560.0;

/// Estado de la paleta abierta.
#[derive(Debug, Clone, Default)]
pub struct Palette {
    pub query: String,
    /// Índice (en la lista filtrada) de la orden resaltada.
    pub selected: usize,
}

/// Una orden de la paleta.
#[derive(Debug, Clone)]
pub struct Command {
    pub label: String,
    pub shortcut: &'static str,
    pub message: Message,
}

impl Command {
    pub fn new(label: impl Into<String>, shortcut: &'static str, message: Message) -> Self {
        Self {
            label: label.into(),
            shortcut,
            message,
        }
    }
}

/// Minúsculas y sin tildes, para comparar sin que importen.
fn fold(c: char) -> char {
    let c = c.to_lowercase().next().unwrap_or(c);
    match c {
        'á' | 'à' | 'ä' | 'â' => 'a',
        'é' | 'è' | 'ë' | 'ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' => 'o',
        'ú' | 'ù' | 'ü' | 'û' => 'u',
        other => other,
    }
}

/// Puntuación difusa: todas las letras de `query` deben aparecer en orden en `candidate`.
/// Puntúan más las letras seguidas y las que empiezan palabra. `None` si no encaja.
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<u32> {
    let query: Vec<char> = query
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(fold)
        .collect();
    if query.is_empty() {
        return Some(0);
    }
    let candidate: Vec<char> = candidate.chars().map(fold).collect();
    let mut score = 0u32;
    let mut next = 0;
    let mut previous_match: Option<usize> = None;
    for wanted in query {
        let index = (next..candidate.len()).find(|&i| candidate.get(i) == Some(&wanted))?;
        score += 1;
        if previous_match.is_some_and(|p| p + 1 == index) {
            score += 5;
        }
        let at_word_start = index == 0
            || candidate
                .get(index - 1)
                .is_some_and(|c| !c.is_alphanumeric());
        if at_word_start {
            score += 8;
        }
        previous_match = Some(index);
        next = index + 1;
    }
    // A igualdad de aciertos, mejor los textos cortos.
    Some(score * 100 / (candidate.len() as u32 + 10).max(1) + score * 10)
}

/// Órdenes que encajan con `query`, de mejor a peor (el orden original desempata).
pub fn filter(commands: Vec<Command>, query: &str) -> Vec<Command> {
    let mut scored: Vec<(u32, usize, Command)> = commands
        .into_iter()
        .enumerate()
        .filter_map(|(index, command)| {
            fuzzy_score(query, &command.label).map(|score| (score, index, command))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, command)| command).collect()
}

/// Capa de la paleta sobre `base`.
pub fn layer<'a>(
    base: Element<'a, Message>,
    palette: &'a Palette,
    matches: &[Command],
) -> Element<'a, Message> {
    let input = text_input("Escribe una orden…", &palette.query)
        .id(Id::new(INPUT_ID))
        .on_input(Message::PaletteInput)
        .on_submit(Message::PaletteRun(None))
        .size(15)
        .padding([8, 10]);

    let mut list = column![].spacing(1);
    for (index, command) in matches.iter().take(MAX_VISIBLE).enumerate() {
        let is_selected = index == palette.selected;
        let item = row![
            text(command.label.clone()).size(13),
            space::horizontal(),
            text(command.shortcut)
                .size(12)
                .style(|theme: &Theme| text::Style {
                    color: Some(theme.extended_palette().background.strong.color),
                }),
        ]
        .align_y(Alignment::Center);
        list = list.push(
            button(item)
                .width(Length::Fill)
                .padding([5, 10])
                .style(move |theme: &Theme, status| {
                    let palette = theme.extended_palette();
                    let mut style = button::text(theme, status);
                    if is_selected || matches!(status, button::Status::Hovered) {
                        style.background = Some(palette.primary.weak.color.into());
                        style.text_color = palette.primary.weak.text;
                    }
                    style
                })
                .on_press(Message::PaletteRun(Some(index))),
        );
    }
    if matches.is_empty() {
        list = list.push(container(text("Ninguna orden coincide").size(13)).padding([5, 10]));
    }

    let panel = container(column![filter_command_keys(input), list].spacing(6))
        .width(WIDTH)
        .padding(8)
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style::default()
                .background(palette.background.base.color)
                .border(iced::Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 8.0.into(),
                })
                .shadow(Shadow {
                    color: iced::Color {
                        a: 0.35,
                        ..iced::Color::BLACK
                    },
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 16.0,
                })
        });
    let positioned = container(panel)
        .width(Length::Fill)
        .padding([60, 0])
        .align_x(Alignment::Center);
    let backdrop = mouse_area(space().width(Length::Fill).height(Length::Fill))
        .on_press(Message::ClosePalette);
    stack![base, opaque(stack![backdrop, positioned])].into()
}

/// Mueve la selección `delta` posiciones dentro de `count` resultados visibles.
pub fn move_selection(selected: usize, delta: i32, count: usize) -> usize {
    let count = count.min(MAX_VISIBLE);
    if count == 0 {
        return 0;
    }
    let current = selected.min(count - 1) as i64;
    let next = (current + i64::from(delta)).rem_euclid(count as i64);
    usize::try_from(next).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_matching() {
        assert!(fuzzy_score("gdr", "Archivo: Guardar").is_some());
        assert!(fuzzy_score("xyz", "Archivo: Guardar").is_none());
        assert_eq!(fuzzy_score("", "lo que sea"), Some(0));
    }

    #[test]
    fn accents_and_case_are_ignored() {
        assert!(fuzzy_score("linea", "Ir a línea").is_some());
        assert!(fuzzy_score("MAYUS", "Editar: MAYÚSCULAS").is_some());
    }

    #[test]
    fn word_starts_and_consecutive_letters_rank_higher() {
        let commands = vec![
            Command::new("Ver: guías de indentación", "", Message::NewDocument),
            Command::new("Archivo: Guardar", "", Message::Save),
        ];
        let result = filter(commands, "guar");
        assert_eq!(result[0].label, "Archivo: Guardar");
    }

    #[test]
    fn selection_wraps() {
        assert_eq!(move_selection(0, -1, 5), 4);
        assert_eq!(move_selection(4, 1, 5), 0);
        assert_eq!(move_selection(3, 1, 0), 0);
        // Solo se puede seleccionar entre los visibles.
        assert_eq!(move_selection(11, 1, 40), 0);
    }
}
