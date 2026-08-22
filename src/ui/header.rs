//! The header row shared by every screen: app identity and pet on the left,
//! timer/day/clock on the right. `header_parts` hands back the two halves so
//! each screen can join them its own way — split wide, or with its own chip.

use crossterm::style::Color;

use super::line::{line_w, seg};
use super::{HomeView, Line};
use crate::i18n;
use crate::pet::Pet;

// Design: dimmed label, only the countdown in yellow.
pub(super) fn timer_segs(view: &HomeView) -> Line {
    view.timer
        .as_ref()
        .map(|(label, t)| {
            vec![seg(format!("{label} "), Some(Color::DarkGrey)), seg(t.clone(), Some(Color::Yellow))]
        })
        .unwrap_or_default()
}

pub(super) fn header_parts(pet: &Pet, view: &HomeView) -> (Line, Line) {
    let (sym, sym_color) = if (6..18).contains(&view.hour) { ("☀", Color::Yellow) } else { ("☾", Color::Blue) };
    let zen = if pet.zen { format!("  ({})", i18n::t().zen_tag) } else { String::new() };
    let left = vec![
        seg(i18n::t().app_title, Some(Color::Magenta)),
        seg(format!("  {}{zen}", pet.name), None),
        seg(
            format!(" · {} · {} {}", i18n::species_name(pet.species), i18n::t().level_short, pet.level),
            Some(Color::DarkGrey),
        ),
    ];
    let mut right: Line = timer_segs(view);
    if !right.is_empty() {
        right.push(seg("   ", None));
    }
    right.push(seg(format!("{sym} "), Some(sym_color)));
    right.push(seg(format!("{} {} · {}", i18n::t().day, pet.day(), view.clock_text), Some(Color::DarkGrey)));
    (left, right)
}

// Design: app identity on the left, day/clock on the right, one row.
pub(super) fn header_split(pet: &Pet, view: &HomeView, w: usize) -> Line {
    let (mut left, right) = header_parts(pet, view);
    let pad = w.saturating_sub(line_w(&left) + line_w(&right));
    left.push(seg(" ".repeat(pad), None));
    left.extend(right);
    left
}

pub(super) fn header_line(pet: &Pet, view: &HomeView) -> Line {
    let (mut left, right) = header_parts(pet, view);
    left.push(seg("   ", None));
    left.extend(right);
    left
}
