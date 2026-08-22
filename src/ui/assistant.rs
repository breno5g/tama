//! Assistant mode: the pet reacting to what an external program said, with the
//! message in a speech bubble and the queue below. Three width tiers, from a
//! full panel layout down to a single row that fits a 26x8 tmux pane.

use std::io::{self, Write};

use crossterm::style::Color;
use crossterm::terminal;

use super::answer::{answer_rows, OPTION_ROWS};
use super::bubble::{bubble_panel, countdown_seg};
use super::expression::{animate_art, animate_tiny, kind_color, kind_face};
use super::header::{header_parts, timer_segs};
use super::line::{beside, ellipsize, line_w, pad_block, plain, seg, tinted, wrapped_text};
use super::panel::{boxed, panel};
use super::screen::draw_screen;
use super::{HomeView, Line};
use crate::assistant::Kind;
use crate::i18n;
use crate::pet::{Mood, Pet};
use crate::species::ArtSize;

#[cfg(test)]
mod tests;

// What draw_assistant shows for the current message.
pub struct AssistantMsg<'a> {
    pub text: &'a str,
    pub from: &'a str,
    pub kind: Kind,
    pub kind_label: &'a str,
    pub options: Option<&'a [String]>,
    pub expires_in: Option<u64>, // seconds until the ask is dropped
    pub input: Option<&'a str>,  // Some = typing a free-text answer right now
    pub input_ok: bool,          // a typed answer is offered as one more option
    pub sel: usize,              // highlighted option (the list scrolls to it)
}

const QUEUE_ROWS: usize = 2;

pub fn draw_assistant(
    out: &mut impl Write,
    pet: &Pet,
    frame: usize,
    msg: Option<&AssistantMsg>,
    queue_preview: &[String],
    queue_len: usize,
    view: &HomeView,
) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let footers: &[&str] = match msg {
        Some(m) if m.input.is_some() => &i18n::t().footer_input,
        Some(m) if m.options.is_some() => &i18n::t().footer_ask,
        _ => &i18n::t().footer_assistant,
    };
    let content = build_assistant(pet, frame, msg, queue_preview, queue_len, view, iw, ih);
    draw_screen(out, &content, footers)
}

pub fn build_assistant(
    pet: &Pet,
    frame: usize,
    msg: Option<&AssistantMsg>,
    queue_preview: &[String],
    queue_len: usize,
    view: &HomeView,
    iw: usize,
    ih: usize,
) -> Vec<Line> {
    // Per-kind expression and animation; a calm happy face while idle.
    let face = msg
        .map(|m| kind_face(m.kind, frame))
        .unwrap_or_else(|| Mood::Happy.face(frame % 4 == 3));
    let kind = msg.map(|m| m.kind);

    let mut content: Vec<Line> = Vec::new();
    if iw >= 72 {
        let w = iw.min(96);
        for size in [ArtSize::Large, ArtSize::Small] {
            let mut art = crate::species::render_art_face(pet.species, size, face.0, face.1);
            if let Some(k) = kind {
                art = animate_art(art, k, frame);
            }
            let right_w = w - art[0].chars().count() - 2;
            let left: Vec<Line> = art.iter().map(|l| plain(l.clone())).collect();
            let mut right = bubble_panel(msg, view.clock_text, right_w);
            let mut queue_body: Vec<Line> =
                queue_preview.iter().take(QUEUE_ROWS).map(|t| tinted(t.clone(), Color::DarkGrey)).collect();
            while queue_body.len() < QUEUE_ROWS {
                queue_body.push(Vec::new());
            }
            right.extend(panel(&format!("{} ({queue_len})", i18n::t().panel_queue), &queue_body, right_w));

            // Design: identity on the left, the "modo assistente" chip on the right.
            let (mut header, _) = header_parts(pet, view);
            let mut chip: Line = timer_segs(view);
            if !chip.is_empty() {
                chip.push(seg("   ", None));
            }
            chip.push((format!(" {} ", i18n::t().assistant_tag), Some(Color::Cyan), Some(Color::DarkGrey)));
            let pad = w.saturating_sub(line_w(&header) + line_w(&chip));
            header.push(seg(" ".repeat(pad), None));
            header.extend(chip);
            let mut c: Vec<Line> = vec![header, Vec::new()];
            c.extend(beside(&left, &right, 2));
            if c.len() + 1 <= ih {
                content = c;
                break;
            }
        }
    }
    if content.is_empty() && iw >= 44 && ih >= 7 {
        // Compact tier, following the design's "pergunta" panel: the face
        // beside a small kind-colored bubble with a tail; asker and options
        // live inside the bubble. Fixed body rows per shape — no reflow.
        let mut face_str = crate::species::render_tiny_face(pet.species, face.0, face.1);
        if let Some(k) = kind {
            face_str = animate_tiny(face_str, k, frame);
        }
        let bubble_w = (iw - face_str.chars().count() - 1).min(58);
        let inner = bubble_w.saturating_sub(4);
        let color = msg.map(|m| kind_color(m.kind)).unwrap_or(Color::DarkGrey);
        let mut body: Vec<Line> = Vec::new();
        match msg {
            // ask: asker + countdown row, 2 text rows, OPTION_ROWS option rows
            Some(m) if m.options.is_some() => {
                let mut first: Line = Vec::new();
                if !m.from.is_empty() {
                    first.push(seg(format!("{} {}:", m.from, i18n::t().asks_verb), Some(Color::DarkGrey)));
                }
                if let Some(e) = m.expires_in {
                    if !first.is_empty() {
                        first.push(seg("  ", None));
                    }
                    first.push(countdown_seg(e));
                }
                body.push(first);
                body.extend(wrapped_text(m.text, inner, 2));
                while body.len() < 3 {
                    body.push(Vec::new());
                }
                body.extend(answer_rows(m, inner));
            }
            Some(m) => {
                body.extend(wrapped_text(m.text, inner, 3));
                while body.len() < 3 {
                    body.push(Vec::new());
                }
                let mut last: Line = Vec::new();
                if !m.from.is_empty() {
                    last.push(seg(format!("{}: ", i18n::t().from_label), Some(Color::DarkGrey)));
                    last.push(seg(m.from, Some(color)));
                    last.push(seg("   ", None));
                }
                last.push(seg(format!("{}: {}", i18n::t().type_label, m.kind_label), Some(Color::DarkGrey)));
                body.push(last);
            }
            None => {
                body.push(tinted(i18n::t().no_messages, Color::DarkGrey));
                while body.len() < 4 {
                    body.push(Vec::new());
                }
            }
        }
        let mut bubble = boxed(None, color, &body, bubble_w);
        bubble[2][0] = seg("< ", Some(color));
        let face_color = kind.map(kind_color).unwrap_or(Color::Green);
        let left: Vec<Line> = vec![Vec::new(), Vec::new(), tinted(face_str, face_color)];
        let mut c = beside(&left, &bubble, 1);
        if queue_len > 0 && c.len() + 2 <= ih {
            c.push(tinted(format!("{} ({queue_len})", i18n::t().panel_queue), Color::DarkGrey));
        }
        if c.len() + 1 <= ih {
            content = c;
        }
    }
    if content.is_empty() {
        // Last resort (Termux 26×8): one header row — face, sender, countdown,
        // queue badge — then text and options split over the height that's left.
        // Options are never sacrificed below what fits; text keeps at least a row.
        let face_color = kind.map(kind_color).unwrap_or(Color::Green);
        let width = iw.max(10).min(60);
        let mut header: Line =
            vec![seg(crate::species::render_tiny_face(pet.species, face.0, face.1), Some(face_color))];
        if let Some(m) = msg {
            if !m.from.is_empty() {
                header.push(seg(format!(" {}", m.from), Some(Color::DarkGrey)));
            }
            if let Some(e) = m.expires_in {
                header.push(seg(" ", None));
                header.push(countdown_seg(e));
            }
        }
        if queue_len > 0 {
            header.push(seg(format!(" +{queue_len}"), Some(Color::DarkGrey)));
        }
        let mut rows: Vec<Line> = vec![ellipsize(header, width)];
        let avail = ih.saturating_sub(1).max(2); // content rows (footer takes one)
        if let Some(m) = msg {
            // slots reserved by shape, not by option count — no reflow between asks
            let opt_rows = if m.options.is_some() { OPTION_ROWS.min(avail.saturating_sub(2)) } else { 0 };
            let text_rows = (avail - 1 - opt_rows).clamp(1, 3);
            rows.extend(wrapped_text(m.text, width, text_rows));
            if m.options.is_some() {
                rows.extend(answer_rows(m, width).into_iter().take(opt_rows.max(1)));
            }
        } else {
            rows.push(tinted(i18n::t().no_messages, Color::DarkGrey));
        }
        rows.truncate(ih.saturating_sub(1).max(1));
        content = pad_block(rows);
    }
    content
}
