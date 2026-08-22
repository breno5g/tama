//! The home screen: the pet in its scene, the status and mood panels, and the
//! event log — laid out down a height ladder so it degrades instead of
//! breaking. Every variable-length section has a reserved height, so a new
//! event or a longer message never reflows the frame.

use std::io::{self, Write};

use crossterm::style::Color;
use crossterm::terminal;

use super::header::{header_line, header_split};
use super::line::{beside, center_in, clip_pad, line_w, pad_block, plain, seg, tinted};
use super::panel::panel;
use super::screen::draw_screen;
use super::stats::{level_color, mood_color, mood_line, stat_bars};
use super::{HomeView, Line};
use crate::i18n;
use crate::pet::{Mood, Pet};
use crate::species::{render_art, render_tiny, ArtSize};

#[cfg(test)]
mod tests;

pub(super) const GRASS: &str = "▁▂▁▁▃▁▂▁▁▁▂▁▃▁▁▂▁▁▁▂▁▃▁▂▁▁";

// The Interface 2.0 panel layout: split header; pet inside a panel titled
// with its name; status and mood panels on the right; events across the
// bottom. `grass`, `tastes` and `event_rows` are the knobs the height ladder
// turns off one by one as the terminal gets shorter.
fn build_panels(
    pet: &Pet,
    frame: usize,
    view: &HomeView,
    w: usize,
    size: ArtSize,
    grass: bool,
    tastes: bool,
    event_rows: usize,
    sky: usize,
) -> Vec<Line> {
    let mood = pet.mood();
    let art = render_art(pet.species, mood, frame, size);

    let right_w = 36;
    let left_w = if pet.zen { w } else { w - right_w - 2 };
    let inner = left_w.saturating_sub(4);

    // `sky` rows of air above the sprite stretch the scene vertically on
    // tall terminals, keeping the pet standing on the grass line.
    let mut pet_body: Vec<Line> = (0..sky).map(|_| Vec::new()).collect();
    // Speech bubble from the design, top-right of the sprite. The row is
    // always reserved (blank while sleeping) so it never reflows the layout.
    let bubble = if mood == Mood::Sleeping { String::new() } else { format!("( {} )", i18n::species_sound(pet.species)) };
    pet_body.push(plain(format!("{bubble:>w$}", w = inner * 2 / 3)));
    pet_body.extend(art.iter().map(|l| center_in(&plain(l.clone()), inner)));
    if grass {
        pet_body.push(center_in(
            &tinted(GRASS.chars().cycle().take(inner.min(40)).collect::<String>(), Color::DarkGreen),
            inner,
        ));
    }
    pet_body.push(center_in(&mood_line(pet), inner));
    let left = panel(&pet.name, &pet_body, left_w);

    let mut content: Vec<Line> = vec![header_split(pet, view, w), Vec::new()];
    if pet.zen {
        content.extend(left);
        content.push(center_in(&tinted(i18n::t().zen_mode, Color::DarkGrey), w));
        return content;
    }

    let mut right: Vec<Line> = panel(i18n::t().panel_status, &stat_bars(pet, false), right_w);
    if tastes {
        let (likes, hates) = i18n::species_tastes(pet.species);
        right.extend(panel(
            i18n::t().panel_mood,
            &[
                plain(i18n::species_trait(pet.species)),
                tinted(format!("{}: {likes}", i18n::t().likes), Color::DarkGrey),
                tinted(format!("{}: {hates}", i18n::t().hates), Color::DarkGrey),
            ],
            right_w,
        ));
    }
    content.extend(beside(&left, &right, 2));

    if event_rows > 0 {
        // Fixed capacity: the panel is always exactly `event_rows` tall,
        // padded with blank rows — new events must never resize the layout.
        let mut events: Vec<Line> = view.progress.iter().take(event_rows).cloned().collect();
        events.extend(view.log.iter().rev().take(event_rows.saturating_sub(events.len())).cloned());
        if events.is_empty() {
            events.push(tinted(i18n::t().log_empty, Color::DarkGrey));
        }
        while events.len() < event_rows {
            events.push(Vec::new());
        }
        content.extend(panel(i18n::t().panel_events, &events, w));
    } else {
        // Ticker row is reserved even before the first event, same reason.
        let ticker = view.progress.first().cloned().or_else(|| view.log.back().cloned());
        content.push(ticker.map(|l| clip_pad(&l, w)).unwrap_or_default());
    }
    content
}

// Stacked fallback that ADDS sections while they fit: art (or tiny face) and
// mood first, then bars, then header, then the last-event ticker.
fn build_stacked(pet: &Pet, frame: usize, view: &HomeView, iw: usize, avail: usize) -> Vec<Line> {
    let mood = pet.mood();
    let art = render_art(pet.species, mood, frame, ArtSize::Small);
    let art_fits = art[0].chars().count() <= iw && art.len() + 1 <= avail;
    let ticker = || view.progress.first().cloned().or_else(|| view.log.back().cloned());

    if art_fits {
        let mut content: Vec<Line> = art.iter().map(|l| plain(l.clone())).collect();
        content.push(mood_line(pet));
        if pet.zen && avail > content.len() {
            content.push(tinted(i18n::t().zen_mode, Color::DarkGrey));
        }
        if !pet.zen {
            let room = avail.saturating_sub(content.len());
            if room >= 6 {
                content.push(Vec::new());
                content.extend(stat_bars(pet, iw < 34));
            } else if room >= 5 {
                content.extend(stat_bars(pet, true));
            }
        }
        if iw >= 40 && avail.saturating_sub(content.len()) >= 2 {
            content.insert(0, header_line(pet, view));
            content.insert(1, Vec::new());
        }
        if !pet.zen && avail.saturating_sub(content.len()) >= 1 {
            content.push(ticker().unwrap_or_default());
        }
        return content;
    }

    // Mini panel, per the design: mood-colored face, name, mood dot and level
    // on one row; then compact bars and the ticker — all padded to one width
    // so the block keeps a single left edge under centering.
    let mc = mood_color(mood);
    let mut face: Line = vec![seg(render_tiny(pet.species, mood, frame), Some(mc))];
    face.push(seg(format!(" {}", pet.name), None));
    face.push(seg(" ●", Some(mc)));
    face.push(seg(
        format!(" {} {}{}", i18n::t().level_short, pet.level, if pet.zen { " · zen" } else { "" }),
        Some(Color::DarkGrey),
    ));
    // reserved trailing slot for the sleeping zzz — appears without reflow
    face.push(seg(
        format!(" {}", if mood == Mood::Sleeping { crate::species::zzz(frame) } else { "     " }),
        Some(Color::Blue),
    ));
    let mut rows: Vec<Line> = vec![face];
    if !pet.zen {
        let room = avail.saturating_sub(rows.len());
        if room >= 5 {
            rows.extend(stat_bars(pet, true));
        } else if room >= 1 {
            let min = pet.hunger.min(pet.happiness).min(pet.energy).min(pet.hygiene);
            rows.push(tinted(
                format!("F{:>3} A{:>3} E{:>3} H{:>3}", pet.hunger, pet.happiness, pet.energy, pet.hygiene),
                level_color(min),
            ));
        }
        if avail.saturating_sub(rows.len()) >= 1 {
            let bw = rows.iter().map(line_w).max().unwrap_or(0);
            rows.push(ticker().map(|l| clip_pad(&l, bw)).unwrap_or_default());
        }
    }
    pad_block(rows)
}

// Height ladder for the panel layout, tallest first: (art size, grass,
// mood panel, event rows).
const PANEL_LADDER: [(ArtSize, bool, bool, usize); 6] = [
    (ArtSize::Large, true, true, 3),
    (ArtSize::Large, false, true, 2),
    (ArtSize::Large, false, false, 1),
    (ArtSize::Small, false, true, 2),
    (ArtSize::Small, false, false, 1),
    (ArtSize::Small, false, false, 0),
];

pub(super) fn build_home(pet: &Pet, frame: usize, view: &HomeView, iw: usize, ih: usize) -> Vec<Line> {
    let avail = ih.saturating_sub(1); // footer row
    if iw >= 72 {
        let w = iw.min(96);
        for (size, grass, tastes, event_rows) in PANEL_LADDER {
            let base = build_panels(pet, frame, view, w, size, grass, tastes, event_rows, 0);
            if base.len() > avail {
                continue;
            }
            // Distribute the leftover height — as a function of the terminal
            // size ONLY, never of how many events exist, so the layout stays
            // put as the log fills: extra event capacity first (up to 8 rows),
            // then vertical room in the pet scene (capped at the art height).
            const EVENT_ROWS_MAX: usize = 8;
            let mut leftover = avail - base.len();
            let mut event_rows = event_rows;
            if event_rows > 0 {
                let extra = leftover.min(EVENT_ROWS_MAX.saturating_sub(event_rows));
                event_rows += extra;
                leftover -= extra;
            }
            let sky = leftover.min(render_art(pet.species, pet.mood(), frame, size).len());
            return build_panels(pet, frame, view, w, size, grass, tastes, event_rows, sky);
        }
    }
    build_stacked(pet, frame, view, iw, avail)
}

pub fn draw_home(out: &mut impl Write, pet: &Pet, frame: usize, view: &HomeView) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let content = build_home(pet, frame, view, iw, ih);
    draw_screen(out, &content, &i18n::t().footer_home)
}
