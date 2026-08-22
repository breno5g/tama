//! The pomodoro screen: a big LCD clock beside the pet, the phase bar and cycle
//! count while running, the preset picker while idle. Every width in the clock
//! column is fixed, so flipping between idle and running changes the digits and
//! nothing else — the clock must not move.

use std::io::{self, Write};

use crossterm::style::Color;
use crossterm::terminal;

use super::bigtime::big_time;
use super::header::header_parts;
use super::home::GRASS;
use super::line::{clip_pad, line_w, pad_block, plain, seg, tinted};
use super::panel::{boxed, panel};
use super::screen::draw_screen;
use super::stats::mood_color;
use super::{HomeView, Line};
use crate::i18n;
use crate::pet::{Mood, Pet};
use crate::species::{render_art, render_tiny, ArtSize};

#[cfg(test)]
mod tests;

// What draw_pomo shows for a running cycle.
pub struct PomoRun {
    pub label: &'static str, // "foco" / "pausa"
    pub focus: bool,
    pub frac: u8, // elapsed % of the current phase
    pub cycle: u32,
}

fn preset_rows(sel: usize) -> Vec<Line> {
    i18n::t().pomo_preset_labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let selected = i == sel;
            vec![
                seg(if selected { "▸ " } else { "  " }, Some(Color::Cyan)),
                seg(format!("{} ", i + 1), Some(Color::Cyan)),
                seg(*label, if selected { Some(Color::Cyan) } else { None }),
            ]
        })
        .collect()
}

// Phase progress + cycle counter, shown under the big clock while running.
// 20 cells + the label fit inside the clock's own width, so the clock column
// never widens because of it.
fn phase_row(run: &PomoRun, accent: Color) -> Line {
    let cells = 20;
    let filled = run.frac as usize * cells / 100;
    vec![
        seg("█".repeat(filled), Some(accent)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(format!("  {} {}", i18n::t().pomo_cycle, run.cycle), Some(Color::DarkGrey)),
    ]
}

fn task_rows(view: &HomeView, rows: usize) -> Vec<Line> {
    let mut tasks: Vec<Line> = view.progress.iter().take(rows).cloned().collect();
    if tasks.is_empty() {
        tasks.push(tinted(i18n::t().pomo_no_tasks, Color::DarkGrey));
    }
    while tasks.len() < rows {
        tasks.push(Vec::new());
    }
    tasks
}

// The dedicated pomodoro screen: a big LCD clock with the pet at its side
// (it sleeps through breaks via the normal sleeping pose), the phase bar and
// cycle count while running, the preset picker while idle, and the active
// progress bars below. Same height-ladder discipline as the home screen.
fn build_pomo(
    pet: &Pet,
    frame: usize,
    view: &HomeView,
    clock: &str,
    run: Option<&PomoRun>,
    sel: usize,
    iw: usize,
    ih: usize,
) -> Vec<Line> {
    let avail = ih.saturating_sub(1); // footer row
    let accent = run.map_or(Color::Cyan, |r| if r.focus { Color::Yellow } else { Color::Blue });
    let title = run.map_or(i18n::t().pomo_title, |r| r.label);
    let clock_art = big_time(clock);
    let clock_w = clock_art.first().map_or(0, |l| l.chars().count());
    let mood = pet.mood();
    let face = || tinted(render_tiny(pet.species, mood, frame), mood_color(mood));

    // The clock column: breathing row, clock, blank, then the phase bar or
    // the presets. Every row is exactly clock_w wide and the column is always
    // the same height, so the clock's position CANNOT change when the state
    // flips between idle and running — only its digits do.
    let mut column: Vec<Line> = vec![Vec::new()];
    column.extend(clock_art.iter().map(|l| tinted(l.clone(), accent)));
    column.push(Vec::new());
    match run {
        Some(r) => column.push(phase_row(r, accent)),
        None => column.extend(preset_rows(sel)),
    }
    while column.len() < clock_art.len() + 2 + i18n::t().pomo_preset_labels.len() {
        column.push(Vec::new());
    }
    let column: Vec<Line> = column.iter().map(|l| clip_pad(l, clock_w)).collect();

    // Full tier: the pet+clock pair centered as one group with EQUAL margins,
    // computed in screen coordinates (compensating the panel's own centering
    // pad, so no stacked flooring skews it). Every width involved is fixed,
    // so state changes cannot move the clock on the x axis.
    if iw >= 72 {
        let w = iw.min(96);
        let inner = w.saturating_sub(4);
        // draw_screen will center the w-wide lines inside iw with this pad:
        let lpad = (iw - w) / 2;
        for art in [Some(ArtSize::Large), Some(ArtSize::Small), None] {
            let pet_block: Vec<Line> = match art {
                Some(size) => {
                    let art = render_art(pet.species, mood, frame, size);
                    let art_w = art[0].chars().count();
                    // same scene furniture as home: a reserved bubble row
                    // (blank while sleeping — no reflow) and grass to stand on
                    let bubble =
                        if mood == Mood::Sleeping { String::new() } else { format!("( {} )", i18n::species_sound(pet.species)) };
                    let mut block: Vec<Line> = vec![plain(format!("{bubble:>art_w$}"))];
                    block.extend(art.iter().map(|l| plain(l.clone())));
                    block.push(tinted(GRASS.chars().cycle().take(art_w).collect::<String>(), Color::DarkGreen));
                    block
                }
                // no room for the full art: the tiny face keeps the pet on
                // screen, sitting beside the clock (assistant-screen pattern)
                None => vec![face()],
            };
            let pet_w = pet_block.iter().map(line_w).max().unwrap_or(0);
            let group_w = pet_w + 2 + clock_w;
            if group_w > inner {
                continue; // this art size doesn't fit beside the clock
            }
            // the group's left edge on SCREEN, then translated to body coords
            let left_w = (iw - group_w) / 2 - lpad - 2 + pet_w + 2;
            // pet and clock group share the vertical center: the shorter
            // block is offset so both midlines meet on the y axis.
            let rows = pet_block.len().max(column.len());
            let pet_top = (rows - pet_block.len()) / 2;
            let col_top = (rows - column.len()) / 2;
            let mut body: Vec<Line> = Vec::new();
            for i in 0..rows {
                let pet_row = i
                    .checked_sub(pet_top)
                    .and_then(|j| pet_block.get(j))
                    .cloned()
                    .unwrap_or_default();
                // pet right-aligned against the column, 2 cols of air between
                let mut l: Line = vec![seg(" ".repeat(left_w - 2 - pet_w), None)];
                let gap = pet_w - line_w(&pet_row) + 2;
                l.extend(pet_row);
                l.push(seg(" ".repeat(gap), None));
                match i.checked_sub(col_top).and_then(|j| column.get(j)) {
                    Some(c) => l.extend(c.iter().cloned()),
                    None => l.push(seg(" ".repeat(clock_w), None)),
                }
                body.push(l);
            }
            let mut content: Vec<Line> = vec![pomo_header(pet, view, w), Vec::new()];
            content.extend(boxed(Some((title, accent)), accent, &body, w));
            content.extend(panel(i18n::t().pomo_tasks, &task_rows(view, 3), w));
            if content.len() <= avail {
                return content;
            }
        }
    }

    // Compact tier: face + title over the clock column, then the tasks. The
    // whole block is exactly clock_w wide (draw_screen centers each line
    // independently), so centering the block IS centering the clock, and the
    // single width keeps one left edge for everything.
    if iw >= clock_w && avail >= clock_art.len() + 3 {
        let mut header: Line = face();
        header.push(seg("  ", None));
        header.push(seg(title, Some(accent)));
        let mut content: Vec<Line> = vec![clip_pad(&header, clock_w)];
        content.extend(column.iter().cloned()); // the breathing row is the air below the face
        if avail >= content.len() + 2 {
            content.push(clip_pad(&Vec::new(), clock_w));
            content.extend(
                task_rows(view, avail - content.len() - 1)
                    .iter()
                    .take(2)
                    .map(|l| clip_pad(l, clock_w)),
            );
        }
        if content.len() <= avail {
            return content;
        }
    }

    // Last resort: face + status on one line, plus the presets or first task.
    let mut status: Line = face();
    status.push(seg(format!(" {title}  "), None));
    status.push(seg(clock.to_string(), Some(accent)));
    let mut content: Vec<Line> = vec![status];
    if run.is_none() {
        content.extend(preset_rows(sel));
    } else if let Some(t) = view.progress.first() {
        content.push(t.clone());
    }
    content.truncate(avail.max(1));
    pad_block(content)
}

// Identity on the left, a "pomodoro" chip on the right — the assistant
// screen's header pattern.
fn pomo_header(pet: &Pet, view: &HomeView, w: usize) -> Line {
    let (mut header, _) = header_parts(pet, view);
    let chip: Line = vec![(format!(" {} ", i18n::t().pomo_title), Some(Color::Cyan), Some(Color::DarkGrey))];
    let pad = w.saturating_sub(line_w(&header) + line_w(&chip));
    header.push(seg(" ".repeat(pad), None));
    header.extend(chip);
    header
}

pub fn draw_pomo(
    out: &mut impl Write,
    pet: &Pet,
    frame: usize,
    view: &HomeView,
    clock: &str,
    run: Option<&PomoRun>,
    sel: usize,
) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let content = build_pomo(pet, frame, view, clock, run, sel, iw, ih);
    let footers: &[&str] =
        if run.is_some() { &i18n::t().footer_pomo_active } else { &i18n::t().footer_pomo_idle };
    draw_screen(out, &content, footers)
}
