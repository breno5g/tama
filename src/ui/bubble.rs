//! The speech bubble: an untitled box in the message's color with a tail
//! pointing at the pet, holding the text, a `from · type · time` meta row and
//! the answer slots. The same height for every message shape.

use crossterm::style::Color;

use super::answer::{answer_rows, OPTION_ROWS};
use super::expression::kind_color;
use super::line::{seg, tinted, wrapped_text};
use super::panel::boxed;
use super::{AssistantMsg, Line, Seg};
use crate::i18n;

const BUBBLE_TEXT_ROWS: usize = 4; // fixed: message length must not resize the layout

// Fixed-width countdown so the ticking never reflows the line.
pub(super) fn countdown_seg(expires_in: u64) -> Seg {
    let color = if expires_in <= 10 { Color::Red } else { Color::Yellow };
    (format!("{} {:>3}s", i18n::t().expires_label, expires_in.min(999)), Some(color), None)
}

// The design's speech bubble: an untitled box in the kind's color with a tail
// pointing at the pet, the message inside, a `de · tipo · hora` meta row and
// OPTION_ROWS option slots. Always the same height for every message shape.
pub(super) fn bubble_panel(msg: Option<&AssistantMsg>, clock_text: &str, w: usize) -> Vec<Line> {
    const BODY_ROWS: usize = BUBBLE_TEXT_ROWS + 2 + OPTION_ROWS; // text + blank + meta + options
    let inner = w.saturating_sub(4);
    let Some(m) = msg else {
        let mut body: Vec<Line> = vec![tinted(i18n::t().no_messages, Color::DarkGrey)];
        while body.len() < BODY_ROWS {
            body.push(Vec::new());
        }
        return boxed(None, Color::DarkGrey, &body, w);
    };

    let color = kind_color(m.kind);
    let mut body: Vec<Line> = Vec::new();
    if m.options.is_some() && !m.from.is_empty() {
        body.push(tinted(format!("{} {}:", m.from, i18n::t().asks_verb), Color::DarkGrey));
    }
    let text_rows = BUBBLE_TEXT_ROWS - body.len();
    body.extend(wrapped_text(m.text, inner, text_rows));
    while body.len() < BUBBLE_TEXT_ROWS {
        body.push(Vec::new());
    }
    body.push(Vec::new());
    let mut meta: Line = Vec::new();
    if !m.from.is_empty() {
        meta.push(seg(format!("{}: ", i18n::t().from_label), Some(Color::DarkGrey)));
        meta.push(seg(m.from, Some(color)));
        meta.push(seg("   ", None));
    }
    meta.push(seg(format!("{}: {}", i18n::t().type_label, m.kind_label), Some(Color::DarkGrey)));
    meta.push(seg(format!("   {clock_text}"), Some(Color::DarkGrey)));
    if let Some(e) = m.expires_in {
        meta.push(seg("   ", None));
        meta.push(countdown_seg(e));
    }
    body.push(meta);
    match m.options {
        Some(_) => body.extend(answer_rows(m, inner)),
        None => body.extend((0..OPTION_ROWS).map(|_| Line::new())),
    }

    let mut rows = boxed(None, color, &body, w);
    // tail toward the pet on the second body row
    if rows.len() > 2 {
        rows[2][0] = seg("< ", Some(color));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_is_fixed_width_and_turns_red_near_expiry() {
        let (t59, c59, _) = countdown_seg(59);
        let (t9, c9, _) = countdown_seg(9);
        let (t_big, ..) = countdown_seg(5000);
        assert_eq!(t59.chars().count(), t9.chars().count());
        assert_eq!(t_big.chars().count(), t59.chars().count());
        assert_eq!(c59, Some(Color::Yellow));
        assert_eq!(c9, Some(Color::Red));
    }
}
