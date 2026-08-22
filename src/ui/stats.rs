//! Stat, xp and progress bars, plus the mood colors they share.

use crossterm::style::Color;

use super::line::{pad_block, seg, tinted};
use super::Line;
use crate::i18n;
use crate::pet::{Mood, Pet};

pub fn level_color(value: u8) -> Color {
    match value {
        0..=29 => Color::Red,
        30..=59 => Color::Yellow,
        _ => Color::Green,
    }
}

fn stat_bar(label: &str, short: char, value: u8, compact: bool) -> Line {
    let cells = if compact { 5 } else { 10 };
    let filled = (value as usize * cells) / 100;
    let c = level_color(value);
    let head = if compact { format!("{short}[") } else { format!("{label:<11}") };
    vec![
        seg(head, None),
        seg("█".repeat(filled), Some(c)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(if compact { format!("]{value:>3}") } else { format!(" {value:>3}") }, Some(c)),
    ]
}

fn xp_bar(pet: &Pet, compact: bool) -> Line {
    let cells = if compact { 5 } else { 10 };
    let need = pet.xp_needed();
    let filled = ((pet.xp * cells as u32) / need) as usize;
    let head = if compact { "x[".to_string() } else { format!("{:<11}", i18n::t().xp_label) };
    vec![
        seg(head, None),
        seg("█".repeat(filled), Some(Color::Cyan)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(if compact { "]".to_string() } else { format!(" {}/{}", pet.xp, need) }, Some(Color::Cyan)),
    ]
}

pub(super) fn stat_bars(pet: &Pet, compact: bool) -> Vec<Line> {
    let values = [pet.hunger, pet.happiness, pet.energy, pet.hygiene];
    let mut bars: Vec<Line> =
        i18n::t().stat_labels
            .iter()
            .zip(i18n::t().stat_short)
            .zip(values)
            .map(|((label, short), v)| stat_bar(label, short, v, compact))
            .collect();
    bars.push(xp_bar(pet, compact));
    pad_block(bars)
}

pub(super) fn mood_line(pet: &Pet) -> Line {
    let mood = pet.mood();
    tinted(format!("● {}", i18n::mood_label(mood)), mood_color(mood))
}

pub fn mood_color(mood: Mood) -> Color {
    match mood {
        Mood::Happy => Color::Green,
        Mood::Hungry | Mood::Dirty => Color::Yellow,
        Mood::Sleepy | Mood::Sleeping => Color::Blue,
        Mood::Sad => Color::Red,
    }
}

// As in the design's progress row: task name, a long green bar, green percent.
pub fn progress_line(from: &str, pct: u8) -> Line {
    let cells = 20usize;
    let filled = (pct as usize * cells) / 100;
    let name = if from.is_empty() { i18n::t().progress_default } else { from };
    vec![
        seg(format!("{name} "), None),
        seg("█".repeat(filled), Some(Color::Green)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(format!(" {pct}%"), Some(Color::Green)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_bars_cover_all_stats_plus_xp() {
        let pet = Pet::default();
        assert_eq!(stat_bars(&pet, false).len(), 5);
    }
}
