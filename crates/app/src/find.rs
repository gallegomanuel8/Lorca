//! Barra de búsqueda y sustitución, barra «Ir a línea» y panel de resultados.
//!
//! Aquí solo hay estado de la interfaz y vistas; la lógica de búsqueda está en
//! `lorca-search` y la aplicación de cambios en `app.rs`.

use std::ops::Range;

use iced::widget::{
    Id, button, checkbox, column, container, pick_list, row, scrollable, space, text, text_input,
    tooltip,
};
use iced::{Alignment, Element, Font, Length, Theme};
use lorca_core::Buffer;
use lorca_search::{SearchMode, SearchOptions};
use lorca_workspace::DocumentId;

use crate::app::Message;
use crate::key_filter::filter_command_keys;

pub const PATTERN_ID: &str = "find-pattern";
pub const REPLACEMENT_ID: &str = "find-replacement";
pub const GOTO_ID: &str = "goto-line";

/// Máximo de coincidencias que se resaltan o se listan.
pub const MAX_MATCHES: usize = 100_000;
/// Máximo de filas que muestra el panel de resultados.
const MAX_RESULT_ROWS: usize = 2_000;
/// Longitud máxima de la vista previa de cada resultado.
const PREVIEW_CHARS: usize = 160;

/// Estado de la barra de búsqueda. Es global (no por documento), como en Notepad++.
#[derive(Debug, Clone)]
pub struct FindState {
    pub open: bool,
    pub pattern: String,
    pub replacement: String,
    pub options: SearchOptions,
    pub wrap: bool,
    /// Buscar solo dentro de este rango (la selección al activar «En selección»).
    pub scope: Option<Range<usize>>,
    /// Al marcar, poner también un marcador en cada línea con coincidencias.
    pub mark_lines: bool,
    /// Error de la consulta (regex no válida…).
    pub error: Option<String>,
    /// Último resultado de una acción («17 coincidencias», «No encontrado»…).
    pub status: Option<String>,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            open: false,
            pattern: String::new(),
            replacement: String::new(),
            options: SearchOptions::default(),
            wrap: true,
            scope: None,
            mark_lines: false,
            error: None,
            status: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FindMessage {
    PatternChanged(String),
    ReplacementChanged(String),
    ToggleCase(bool),
    ToggleWord(bool),
    ModeSelected(SearchMode),
    ToggleDotNewline(bool),
    ToggleInSelection(bool),
    ToggleWrap(bool),
    ToggleMarkLines(bool),
    /// Enter en el campo de búsqueda (con Mayús, hacia atrás).
    Submit,
    Next,
    Previous,
    Count,
    FindAll {
        all_documents: bool,
    },
    Replace,
    ReplaceAll,
    MarkAll,
    ClearMarks,
    CopyBookmarkedLines,
    DeleteBookmarkedLines,
    ClearBookmarks,
    Close,
    /// Abre el resultado `hit` del grupo `group` del panel.
    OpenHit {
        group: usize,
        hit: usize,
    },
    CloseResults,
    GotoChanged(String),
    GotoSubmit,
}

/// Resultados de «Buscar todo».
#[derive(Debug, Clone)]
pub struct SearchResults {
    pub pattern: String,
    pub groups: Vec<ResultGroup>,
    pub total: usize,
}

/// Coincidencias de un documento.
#[derive(Debug, Clone)]
pub struct ResultGroup {
    pub id: DocumentId,
    pub name: String,
    /// Revisión del documento al buscar: si cambia, se salta a la línea en vez de al rango.
    pub revision: u64,
    pub hits: Vec<Hit>,
}

/// Una coincidencia lista para mostrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub line: usize,
    /// Rango en el documento (caracteres).
    pub range: Range<usize>,
    /// Texto de la línea (recortado) y la parte que coincide dentro de él.
    pub preview: String,
    pub highlight: Range<usize>,
}

/// Prepara las filas de resultados a partir de las coincidencias de un documento.
pub fn build_hits(buffer: &Buffer, matches: &[Range<usize>]) -> Vec<Hit> {
    matches
        .iter()
        .map(|range| {
            let position = buffer.char_to_position(range.start);
            let line: Vec<char> = buffer.line_text(position.line).chars().collect();
            let match_len = range.len().min(line.len().saturating_sub(position.column));
            // Ventana de texto alrededor de la coincidencia.
            let first = position.column.saturating_sub(PREVIEW_CHARS / 3);
            let last = (position.column + match_len + PREVIEW_CHARS).min(line.len());
            let preview: String = line.get(first..last).unwrap_or_default().iter().collect();
            let start = position.column - first;
            Hit {
                line: position.line,
                range: range.clone(),
                preview: preview.replace('\t', " "),
                highlight: start..start + match_len,
            }
        })
        .collect()
}

/// Interpreta «línea» o «línea:columna» (desde 1). Devuelve índices desde cero.
pub fn parse_goto(input: &str) -> Option<(usize, usize)> {
    let mut parts = input.trim().splitn(2, [':', ',']);
    let line: usize = parts.next()?.trim().parse().ok()?;
    let column: usize = match parts.next() {
        Some(column) => column.trim().parse().ok()?,
        None => 1,
    };
    Some((line.checked_sub(1)?, column.saturating_sub(1)))
}

// --- Vistas ---------------------------------------------------------------------------

fn find(message: FindMessage) -> Message {
    Message::Find(message)
}

/// Barra de búsqueda. `counter` es «3 de 17», «17 coincidencias», etc.
pub fn bar<'a>(state: &'a FindState, counter: String, bookmarks: usize) -> Element<'a, Message> {
    let options = state.options;
    let small = |label: &'a str, message: FindMessage| {
        button(text(label).size(12))
            .padding([3, 8])
            .style(button::secondary)
            .on_press(find(message))
    };
    let toggle = |label: &'a str, checked: bool, message: fn(bool) -> FindMessage| {
        checkbox(checked)
            .label(label)
            .text_size(12)
            .size(14)
            .on_toggle(move |value| find(message(value)))
    };

    let pattern = text_input("Buscar", &state.pattern)
        .id(Id::new(PATTERN_ID))
        .on_input(|value| find(FindMessage::PatternChanged(value)))
        .on_submit(find(FindMessage::Submit))
        .size(13)
        .padding([4, 8])
        .width(Length::FillPortion(3));
    let replacement = text_input("Reemplazar con", &state.replacement)
        .id(Id::new(REPLACEMENT_ID))
        .on_input(|value| find(FindMessage::ReplacementChanged(value)))
        .on_submit(find(FindMessage::Replace))
        .size(13)
        .padding([4, 8])
        .width(Length::FillPortion(3));

    let status_text = match (&state.error, &state.status) {
        (Some(error), _) => error.clone(),
        (None, Some(status)) => status.clone(),
        (None, None) => counter,
    };
    let is_error = state.error.is_some();
    let status = text(status_text).size(12).style(move |theme: &Theme| {
        let palette = theme.extended_palette();
        text::Style {
            color: Some(if is_error {
                palette.danger.base.color
            } else {
                palette.background.strong.text
            }),
        }
    });

    let mut options_row = row![
        pick_list(SearchMode::ALL, Some(options.mode), |mode| find(
            FindMessage::ModeSelected(mode)
        ))
        .text_size(12)
        .padding([2, 6]),
        toggle("Aa", options.match_case, FindMessage::ToggleCase),
        toggle("Palabra", options.whole_word, FindMessage::ToggleWord),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    if options.mode == SearchMode::Regex {
        options_row = options_row.push(toggle(
            ". = \\n",
            options.dot_matches_newline,
            FindMessage::ToggleDotNewline,
        ));
    }
    options_row = options_row
        .push(toggle(
            "En selección",
            state.scope.is_some(),
            FindMessage::ToggleInSelection,
        ))
        .push(toggle("Dar la vuelta", state.wrap, FindMessage::ToggleWrap));

    let first = row![
        filter_command_keys(pattern),
        status.width(Length::Fixed(150.0)),
        with_tip(small("↑", FindMessage::Previous), "Anterior (⇧F3)"),
        with_tip(small("↓", FindMessage::Next), "Siguiente (F3)"),
        small("Contar", FindMessage::Count),
        small(
            "Buscar todo",
            FindMessage::FindAll {
                all_documents: false
            }
        ),
        with_tip(
            small(
                "En abiertos",
                FindMessage::FindAll {
                    all_documents: true
                }
            ),
            "Buscar todo en todos los documentos abiertos"
        ),
        button(text("×").size(14))
            .padding([0, 6])
            .style(button::text)
            .on_press(find(FindMessage::Close)),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let second = row![
        filter_command_keys(replacement),
        space().width(Length::Fixed(150.0)),
        small("Reemplazar", FindMessage::Replace),
        small("Reemplazar todo", FindMessage::ReplaceAll),
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let marks = row![
        options_row,
        space::horizontal(),
        small("Marcar todo", FindMessage::MarkAll),
        toggle(
            "Marcar líneas",
            state.mark_lines,
            FindMessage::ToggleMarkLines
        ),
        small("Borrar marcas", FindMessage::ClearMarks),
        text(format!("Marcadores: {bookmarks}")).size(12),
        with_tip(
            small("Copiar", FindMessage::CopyBookmarkedLines),
            "Copiar las líneas marcadas"
        ),
        with_tip(
            small("Borrar", FindMessage::DeleteBookmarkedLines),
            "Borrar las líneas marcadas"
        ),
        with_tip(
            small("Quitar", FindMessage::ClearBookmarks),
            "Quitar todos los marcadores"
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    panel(column![first, second, marks].spacing(6))
}

/// Barra «Ir a línea».
pub fn goto_bar<'a>(input: &'a str, line_count: usize) -> Element<'a, Message> {
    let field = text_input("línea o línea:columna", input)
        .id(Id::new(GOTO_ID))
        .on_input(|value| find(FindMessage::GotoChanged(value)))
        .on_submit(find(FindMessage::GotoSubmit))
        .size(13)
        .padding([4, 8])
        .width(Length::Fixed(220.0));
    panel(
        row![
            text("Ir a línea").size(13),
            filter_command_keys(field),
            text(format!("(1 – {line_count})")).size(12),
            space::horizontal(),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
}

/// Panel inferior con los resultados de «Buscar todo».
pub fn results_panel(results: &SearchResults) -> Element<'_, Message> {
    let documents = results.groups.len();
    let header = row![
        text(format!(
            "Buscar «{}»: {} {} en {} {}",
            results.pattern,
            results.total,
            if results.total == 1 {
                "coincidencia"
            } else {
                "coincidencias"
            },
            documents,
            if documents == 1 {
                "documento"
            } else {
                "documentos"
            },
        ))
        .size(12),
        space::horizontal(),
        button(text("×").size(14))
            .padding([0, 6])
            .style(button::text)
            .on_press(find(FindMessage::CloseResults)),
    ]
    .align_y(Alignment::Center);

    let mut rows: Vec<Element<'_, Message>> = Vec::new();
    let mut shown = 0;
    'groups: for (group_index, group) in results.groups.iter().enumerate() {
        rows.push(
            text(format!("{} ({})", group.name, group.hits.len()))
                .size(12)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Font::default()
                })
                .into(),
        );
        for (hit_index, hit) in group.hits.iter().enumerate() {
            if shown >= MAX_RESULT_ROWS {
                rows.push(
                    text(format!("… y {} más", results.total - shown))
                        .size(12)
                        .into(),
                );
                break 'groups;
            }
            shown += 1;
            rows.push(hit_row(group_index, hit_index, hit));
        }
    }

    let list = scrollable(column(rows).spacing(1).padding([0, 8])).height(Length::Fill);
    container(column![header, list].spacing(4))
        .padding([4, 8])
        .height(Length::Fixed(190.0))
        .width(Length::Fill)
        .style(|theme: &Theme| {
            container::Style::default()
                .background(theme.extended_palette().background.weakest.color)
        })
        .into()
}

fn hit_row(group: usize, index: usize, hit: &Hit) -> Element<'_, Message> {
    let chars: Vec<char> = hit.preview.chars().collect();
    let part =
        |range: Range<usize>| -> String { chars.get(range).unwrap_or_default().iter().collect() };
    let before = part(0..hit.highlight.start);
    let matched = part(hit.highlight.clone());
    let after = part(hit.highlight.end..chars.len());
    let mono = |content: String| text(content).size(12).font(Font::MONOSPACE);

    let content = row![
        mono(format!("{:>6}: ", hit.line + 1)).style(|theme: &Theme| text::Style {
            color: Some(theme.extended_palette().background.strong.color),
        }),
        mono(before),
        mono(matched)
            .font(Font {
                weight: iced::font::Weight::Bold,
                ..Font::MONOSPACE
            })
            .style(|theme: &Theme| text::Style {
                color: Some(theme.extended_palette().primary.strong.color)
            }),
        mono(after),
    ];
    button(content)
        .padding([1, 4])
        .width(Length::Fill)
        .style(button::text)
        .on_press(find(FindMessage::OpenHit { group, hit: index }))
        .into()
}

fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding([6, 10])
        .width(Length::Fill)
        .style(|theme: &Theme| {
            container::Style::default()
                .background(theme.extended_palette().background.weakest.color)
        })
        .into()
}

fn with_tip<'a>(content: impl Into<Element<'a, Message>>, tip: &'a str) -> Element<'a, Message> {
    tooltip(
        content,
        container(text(tip).size(12))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .into()
}

#[cfg(test)]
#[allow(clippy::single_range_in_vec_init)] // Las listas de un solo rango son intencionadas.
mod tests {
    use super::*;

    #[test]
    fn goto_parsing() {
        assert_eq!(parse_goto("12"), Some((11, 0)));
        assert_eq!(parse_goto(" 3:7 "), Some((2, 6)));
        assert_eq!(parse_goto("3,7"), Some((2, 6)));
        assert_eq!(parse_goto("0"), None);
        assert_eq!(parse_goto("abc"), None);
        assert_eq!(parse_goto(""), None);
    }

    #[test]
    fn hits_have_line_and_highlight() {
        let buffer = Buffer::from_text("uno\n\tdos verde tres");
        let hits = build_hits(&buffer, &[9..14]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[0].preview, " dos verde tres");
        let matched: String = hits[0]
            .preview
            .chars()
            .skip(hits[0].highlight.start)
            .take(hits[0].highlight.len())
            .collect();
        assert_eq!(matched, "verde");
    }

    #[test]
    fn long_lines_are_trimmed_around_the_match() {
        let line = format!("{}aguja{}", "x".repeat(1000), "y".repeat(1000));
        let buffer = Buffer::from_text(&line);
        let hits = build_hits(&buffer, &[1000..1005]);
        assert!(hits[0].preview.chars().count() <= PREVIEW_CHARS + PREVIEW_CHARS / 3 + 5);
        let matched: String = hits[0]
            .preview
            .chars()
            .skip(hits[0].highlight.start)
            .take(5)
            .collect();
        assert_eq!(matched, "aguja");
    }

    #[test]
    fn multiline_match_highlight_stops_at_line_end() {
        let buffer = Buffer::from_text("ab\ncd");
        let hits = build_hits(&buffer, &[1..4]);
        assert_eq!(hits[0].highlight, 1..2);
    }
}
