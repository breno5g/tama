//! The answer widgets of an ask: the numbered option list (which scrolls inside
//! a fixed slot) and the free-text field that replaces it while typing.
//!
//! Both occupy exactly OPTION_ROWS rows, always. An ask with two options and an
//! ask with nine must render the same size card, or the bubble jumps as
//! questions arrive.

use crossterm::style::Color;

use super::line::{chip, ellipsize, line_w, seg};
use super::{AssistantMsg, Line};
use crate::i18n;

pub(super) const OPTION_ROWS: usize = 3; // fixed: option count must not resize the layout

// Breaks the buffer into field-width lines, preserving every character
// (spaces included — this is text being typed, not prose being laid out) and
// honouring the newlines the writer put there.
fn input_lines(buf: &str, w: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in buf.split('\n') {
        let mut chars = para.chars().peekable();
        loop {
            out.push(chars.by_ref().take(w.max(1)).collect::<String>());
            if chars.peek().is_none() {
                break;
            }
        }
    }
    out
}

// The typed answer over `rows` lines, following the caret: what scrolled out
// of view above is flagged with ↑, so a long answer is written comfortably in
// a card that never changes size.
pub(super) fn input_rows(buf: &str, w: usize, rows: usize) -> Vec<Line> {
    let inner = w.saturating_sub(2); // room for the "> " gutter
    let lines = input_lines(buf, inner.saturating_sub(1).max(1)); // and the caret
    let hidden = lines.len().saturating_sub(rows);
    let mut out: Vec<Line> = Vec::new();
    for (i, text) in lines.iter().skip(hidden).enumerate() {
        let gutter = match (i, hidden) {
            (0, 0) => "> ",
            (0, _) => "↑ ",
            _ => "  ",
        };
        let mut row: Line = vec![seg(gutter, Some(Color::DarkGrey)), seg(text.clone(), None)];
        if i + hidden + 1 == lines.len() {
            row.push(seg("_", Some(Color::Cyan))); // the caret rides the last line
        }
        out.push(row);
    }
    while out.len() < rows {
        out.push(Vec::new());
    }
    out
}

// Rows an ask occupies below its text: the typing field replaces the options
// while it is open (same slot count either way — no reflow).
pub(super) fn answer_rows(m: &AssistantMsg, w: usize) -> Vec<Line> {
    match m.input {
        Some(buf) => input_rows(buf, w, OPTION_ROWS),
        None => option_rows(&option_labels(m.options.unwrap_or_default(), m.input_ok), m.sel, w),
    }
}

// The choices as shown: the fixed options plus, when free text is accepted,
// one more numbered entry for it — the "Other" of the harness prompts. It
// only fits while there is a key left (1-9).
pub fn option_labels(options: &[String], input_ok: bool) -> Vec<String> {
    let mut labels: Vec<String> = options.iter().take(9).cloned().collect();
    if input_ok && labels.len() < 9 {
        labels.push(i18n::t().option_write.to_string());
    }
    labels
}

// The window of options around the cursor: the list scrolls inside a fixed
// OPTION_ROWS-tall slot, so a long list never resizes the card.
fn option_window(len: usize, sel: usize) -> std::ops::Range<usize> {
    let start = sel.saturating_sub(OPTION_ROWS - 1).min(len.saturating_sub(OPTION_ROWS));
    start..(start + OPTION_ROWS).min(len)
}

// One option per row, cursor-marked like the actions menu, blank-padded to
// OPTION_ROWS. `↑`/`↓ +N` show what is scrolled out of view; anything wider
// than `w` clips with a trailing …
pub(super) fn option_rows(options: &[String], sel: usize, w: usize) -> Vec<Line> {
    let window = option_window(options.len(), sel);
    let (above, below) = (window.start, options.len().saturating_sub(window.end));
    let mut rows: Vec<Line> = Vec::new();
    for i in window {
        let selected = i == sel;
        let mut row: Line = vec![
            seg(if selected { "▸" } else { " " }, Some(Color::Cyan)),
            chip(&(i + 1).to_string()),
            seg(format!(" {}", options[i]), if selected { Some(Color::Cyan) } else { None }),
        ];
        // scroll hints ride the first and last visible rows
        let hint = match (rows.is_empty(), above, below) {
            (true, n, _) if n > 0 => Some(format!(" ↑{n}")),
            (false, _, n) if n > 0 && rows.len() + 1 == OPTION_ROWS => Some(format!(" ↓{n}")),
            _ => None,
        };
        if let Some(h) = hint {
            let pad = w.saturating_sub(line_w(&row) + h.chars().count());
            row.push(seg(" ".repeat(pad), None));
            row.push(seg(h, Some(Color::DarkGrey)));
        }
        rows.push(row);
    }
    while rows.len() < OPTION_ROWS {
        rows.push(Vec::new());
    }
    rows.into_iter().map(|r| ellipsize(r, w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_wraps_without_losing_a_single_character() {
        // char-exact: spaces and doubled blanks survive the round trip
        let buf = "duas  palavras e mais texto";
        assert_eq!(input_lines(buf, 10).concat(), buf);
        // explicit newlines start a new line, empty ones included
        assert_eq!(input_lines("a\n\nb", 10), vec!["a".to_string(), String::new(), "b".to_string()]);
    }

    #[test]
    fn input_rows_follow_the_caret_and_flag_what_scrolled_off() {
        let text = |r: &Line| r.iter().map(|(s, ..)| s.clone()).collect::<String>();
        // short answer: gutter, text, caret — the rest of the slots stay empty
        let rows = input_rows("oi", 20, OPTION_ROWS);
        assert_eq!(rows.len(), OPTION_ROWS);
        assert_eq!(text(&rows[0]), "> oi_");
        assert_eq!(line_w(&rows[2]), 0);
        // long answer: fills the rows, caret on the last, ↑ where it scrolled
        let rows = input_rows(&"a".repeat(200), 20, OPTION_ROWS);
        assert!(text(&rows[0]).starts_with('↑'), "{:?}", text(&rows[0]));
        assert!(text(&rows[2]).ends_with('_'));
        assert!(rows.iter().all(|r| line_w(r) <= 20), "estourou a largura");
        // multi-line answers keep their shape
        let rows = input_rows("linha um\nlinha dois", 20, OPTION_ROWS);
        assert_eq!(text(&rows[0]), "> linha um");
        assert_eq!(text(&rows[1]), "  linha dois_");
    }

    #[test]
    fn option_rows_mark_the_cursor_and_clip_with_ellipsis() {
        let opts: Vec<String> = vec!["curta".into(), "uma opção comprida demais para caber".into()];
        let rows = option_rows(&opts, 0, 16);
        assert_eq!(rows.len(), OPTION_ROWS);
        let first: String = rows[0].iter().map(|(s, ..)| s.clone()).collect();
        assert!(first.starts_with('▸'), "cursor deveria marcar a opção 0: {first:?}");
        let second: String = rows[1].iter().map(|(s, ..)| s.clone()).collect();
        assert!(!second.starts_with('▸'));
        assert!(second.ends_with('…'), "opção longa deveria cortar: {second:?}");
        assert_eq!(line_w(&rows[1]), 16);
        assert_eq!(line_w(&rows[2]), 0); // slot vazio segue reservado
    }

    #[test]
    fn long_option_lists_scroll_around_the_cursor() {
        let many: Vec<String> = (1..=7).map(|i| format!("opção {i}")).collect();
        // no topo: 3 primeiras, com aviso do que ficou abaixo
        assert_eq!(option_window(7, 0), 0..3);
        let rows = option_rows(&many, 0, 40);
        let text = |r: &Line| r.iter().map(|(s, ..)| s.clone()).collect::<String>();
        assert!(text(&rows[0]).contains("opção 1"));
        assert!(text(&rows[2]).contains("↓4"), "faltou o aviso de scroll: {:?}", text(&rows[2]));
        // no fim: janela desliza e o aviso vira o de cima
        assert_eq!(option_window(7, 6), 4..7);
        let rows = option_rows(&many, 6, 40);
        assert!(text(&rows[0]).contains("↑4"), "{:?}", text(&rows[0]));
        assert!(text(&rows[2]).contains("▸") && text(&rows[2]).contains("opção 7"));
        // lista curta não rola nem avisa
        let few: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(option_window(2, 1), 0..2);
        let rows = option_rows(&few, 1, 40);
        assert!(!text(&rows[0]).contains('↑') && !text(&rows[1]).contains('↓'));
    }
}
