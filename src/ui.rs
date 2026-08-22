use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, execute, queue, terminal};

use crate::assistant::Kind;
use crate::i18n;
use crate::pet::{Mood, Pet, FOODS};
use crate::species::{render_art, render_tiny, ArtSize};

pub type Seg = (String, Option<Color>, Option<Color>); // text, fg, bg
pub type Line = Vec<Seg>;

pub fn seg(s: impl Into<String>, c: Option<Color>) -> Seg {
    (s.into(), c, None)
}

// A key cap as in the design: filled slate box, cyan key.
pub fn chip(key: &str) -> Seg {
    (format!(" {key} "), Some(Color::Cyan), Some(Color::DarkGrey))
}

// Converts "[f] comer  [p] brincar" into chip segments + grey labels.
fn footer_line(s: &str) -> Line {
    let mut out: Line = Vec::new();
    let mut text = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '[' {
            if !text.is_empty() {
                out.push(seg(std::mem::take(&mut text), Some(Color::DarkGrey)));
            }
            let mut key = String::new();
            for k in chars.by_ref() {
                if k == ']' {
                    break;
                }
                key.push(k);
            }
            out.push(chip(&key));
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        out.push(seg(text, Some(Color::DarkGrey)));
    }
    out
}

pub fn plain(s: impl Into<String>) -> Line {
    vec![seg(s, None)]
}

pub fn tinted(s: impl Into<String>, c: Color) -> Line {
    vec![seg(s, Some(c))]
}

pub fn line_w(l: &Line) -> usize {
    l.iter().map(|(s, ..)| s.chars().count()).sum()
}

// Joins two blocks of lines side by side. Every resulting line is padded to
// the same total width — draw_screen centers each line independently, so any
// width variation would shift rows against each other and shear the art.
pub fn beside(left: &[Line], right: &[Line], gap: usize) -> Vec<Line> {
    let lw = left.iter().map(line_w).max().unwrap_or(0);
    let rw = right.iter().map(line_w).max().unwrap_or(0);
    (0..left.len().max(right.len()))
        .map(|i| {
            let mut l: Line = left.get(i).cloned().unwrap_or_default();
            l.push(seg(" ".repeat(lw + gap - line_w(&l)), None));
            if let Some(r) = right.get(i) {
                l.extend(r.iter().cloned());
            }
            let tail = lw + gap + rw - line_w(&l);
            if tail > 0 {
                l.push(seg(" ".repeat(tail), None));
            }
            l
        })
        .collect()
}

// Pads a block of lines to a uniform width so it stays internally aligned
// under draw_screen's per-line centering.
pub fn pad_block(mut lines: Vec<Line>) -> Vec<Line> {
    let w = lines.iter().map(line_w).max().unwrap_or(0);
    for l in &mut lines {
        let pad = w - line_w(l);
        if pad > 0 {
            l.push(seg(" ".repeat(pad), None));
        }
    }
    lines
}

// Truncates a line to exactly `w` chars, padding with spaces when shorter.
fn clip_pad(line: &Line, w: usize) -> Line {
    let mut out: Line = Vec::new();
    let mut budget = w;
    for (s, fg, bg) in line {
        if budget == 0 {
            break;
        }
        let t: String = s.chars().take(budget).collect();
        budget -= t.chars().count();
        out.push((t, *fg, *bg));
    }
    if budget > 0 {
        out.push(seg(" ".repeat(budget), None));
    }
    out
}

fn center_in(line: &Line, w: usize) -> Line {
    let lw = line_w(line);
    if lw >= w {
        return clip_pad(line, w);
    }
    let lpad = (w - lw) / 2;
    let mut out: Line = vec![seg(" ".repeat(lpad), None)];
    out.extend(line.iter().cloned());
    out.push(seg(" ".repeat(w - lw - lpad), None));
    out
}

// A titled box, as in the Interface 2.0 design: ┌─ title ────┐ … └────┘.
fn panel(title: &str, body: &[Line], w: usize) -> Vec<Line> {
    boxed(Some((title, Color::DarkGrey)), Color::DarkGrey, body, w)
}

fn boxed(title: Option<(&str, Color)>, border_color: Color, body: &[Line], w: usize) -> Vec<Line> {
    let border = Some(border_color);
    let inner = w.saturating_sub(4);
    let top = match title {
        Some((title, title_color)) => {
            let title: String = title.chars().take(w.saturating_sub(6)).collect();
            let dash = w.saturating_sub(title.chars().count() + 5);
            vec![
                seg("┌─ ", border),
                seg(title, Some(title_color)),
                seg(format!(" {}┐", "─".repeat(dash)), border),
            ]
        }
        None => vec![seg(format!("┌{}┐", "─".repeat(w.saturating_sub(2))), border)],
    };
    let mut out: Vec<Line> = vec![top];
    for b in body {
        let mut l: Line = vec![seg("│ ", border)];
        l.extend(clip_pad(b, inner));
        l.push(seg(" │", border));
        out.push(l);
    }
    out.push(vec![seg(format!("└{}┘", "─".repeat(w.saturating_sub(2))), border)]);
    out
}

fn print_line(out: &mut impl Write, row: u16, iw: usize, line: &Line) -> io::Result<()> {
    let total = line_w(line).min(iw);
    let lpad = (iw - total) / 2;
    let rpad = iw - total - lpad;
    queue!(out, cursor::MoveTo(1, row), Print(" ".repeat(lpad)))?;
    let mut budget = total;
    for (s, fg, bg) in line {
        if budget == 0 {
            break;
        }
        let t: String = s.chars().take(budget).collect();
        budget -= t.chars().count();
        if let Some(c) = fg {
            queue!(out, SetForegroundColor(*c))?;
        }
        if let Some(c) = bg {
            queue!(out, SetBackgroundColor(*c))?;
        }
        queue!(out, Print(t), ResetColor)?;
    }
    queue!(out, Print(" ".repeat(rpad)))
}

// Draws the border, centers `content` in the inner area and pins the widest
// fitting footer candidate to the bottom inner row. Never clears the screen:
// every cell of every frame is repainted, so the previous frame is overwritten
// in place — no blank state, no flicker, even in terminals/tmux without
// synchronized-update support. One flush per frame.
pub fn draw_screen(out: &mut impl Write, content: &[Line], footers: &[&str]) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    queue!(out, terminal::BeginSynchronizedUpdate)?;
    if cols < 4 || rows < 3 {
        queue!(out, terminal::Clear(terminal::ClearType::All), terminal::EndSynchronizedUpdate)?;
        return out.flush();
    }

    let iw = cols as usize - 2;
    let ih = rows as usize - 2;
    let horiz = "─".repeat(iw);
    queue!(out, cursor::MoveTo(0, 0))?;
    queue!(out, SetForegroundColor(Color::DarkGrey), Print(format!("┌{horiz}┐")), ResetColor)?;
    queue!(out, cursor::MoveTo(0, rows - 1))?;
    queue!(out, SetForegroundColor(Color::DarkGrey), Print(format!("└{horiz}┘")), ResetColor)?;

    let footer = footers.iter().find(|f| f.chars().count() <= iw);
    let avail = ih - footer.map_or(0, |_| 1);
    let shown = &content[..content.len().min(avail)];
    let top = avail.saturating_sub(shown.len()) / 2;
    let empty: Line = Vec::new();

    for r in 0..ih {
        let row = (r + 1) as u16;
        queue!(out, cursor::MoveTo(0, row))?;
        queue!(out, SetForegroundColor(Color::DarkGrey), Print("│"), ResetColor)?;
        let line: Line;
        let l = if footer.is_some() && r == ih - 1 {
            line = footer_line(footer.unwrap());
            &line
        } else {
            match r.checked_sub(top).filter(|i| *i < shown.len()) {
                Some(i) => &shown[i],
                None => &empty,
            }
        };
        print_line(out, row, iw, l)?;
        queue!(out, cursor::MoveTo(cols - 1, row))?;
        queue!(out, SetForegroundColor(Color::DarkGrey), Print("│"), ResetColor)?;
    }
    queue!(out, terminal::EndSynchronizedUpdate)?;
    out.flush()
}

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
    let head = if compact { "x[".to_string() } else { format!("{:<11}", i18n::XP_LABEL) };
    vec![
        seg(head, None),
        seg("█".repeat(filled), Some(Color::Cyan)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(if compact { "]".to_string() } else { format!(" {}/{}", pet.xp, need) }, Some(Color::Cyan)),
    ]
}

fn stat_bars(pet: &Pet, compact: bool) -> Vec<Line> {
    let values = [pet.hunger, pet.happiness, pet.energy, pet.hygiene];
    let mut bars: Vec<Line> =
        i18n::STAT_LABELS
            .iter()
            .zip(i18n::STAT_SHORT)
            .zip(values)
            .map(|((label, short), v)| stat_bar(label, short, v, compact))
            .collect();
    bars.push(xp_bar(pet, compact));
    pad_block(bars)
}

// Everything draw_home needs beyond the pet itself.
pub struct HomeView<'a> {
    pub log: &'a VecDeque<Line>,
    pub clock_text: &'a str,
    pub hour: u8,
    pub timer: Option<(&'static str, String)>, // label ("timer"/"foco"/"pausa"), countdown
    pub progress: Vec<Line>,
}

// Design: dimmed label, only the countdown in yellow.
fn timer_segs(view: &HomeView) -> Line {
    view.timer
        .as_ref()
        .map(|(label, t)| {
            vec![seg(format!("{label} "), Some(Color::DarkGrey)), seg(t.clone(), Some(Color::Yellow))]
        })
        .unwrap_or_default()
}

fn header_parts(pet: &Pet, view: &HomeView) -> (Line, Line) {
    let (sym, sym_color) = if (6..18).contains(&view.hour) { ("☀", Color::Yellow) } else { ("☾", Color::Blue) };
    let zen = if pet.zen { format!("  ({})", i18n::ZEN_TAG) } else { String::new() };
    let left = vec![
        seg(i18n::APP_TITLE, Some(Color::Magenta)),
        seg(format!("  {}{zen}", pet.name), None),
        seg(
            format!(" · {} · {} {}", i18n::species_name(pet.species), i18n::LEVEL_SHORT, pet.level),
            Some(Color::DarkGrey),
        ),
    ];
    let mut right: Line = timer_segs(view);
    if !right.is_empty() {
        right.push(seg("   ", None));
    }
    right.push(seg(format!("{sym} "), Some(sym_color)));
    right.push(seg(format!("{} {} · {}", i18n::DAY, pet.day(), view.clock_text), Some(Color::DarkGrey)));
    (left, right)
}

// Design: app identity on the left, day/clock on the right, one row.
fn header_split(pet: &Pet, view: &HomeView, w: usize) -> Line {
    let (mut left, right) = header_parts(pet, view);
    let pad = w.saturating_sub(line_w(&left) + line_w(&right));
    left.push(seg(" ".repeat(pad), None));
    left.extend(right);
    left
}

fn header_line(pet: &Pet, view: &HomeView) -> Line {
    let (mut left, right) = header_parts(pet, view);
    left.push(seg("   ", None));
    left.extend(right);
    left
}

pub fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::Info => Color::Cyan,
        Kind::Success => Color::Green,
        Kind::Warn => Color::Yellow,
        Kind::Error => Color::Red,
    }
}

// The pet reacts to what it is saying, per the design's expression map:
// info = calm, success = happy, warn = wide eyes (no blink), error = sad.
pub fn kind_face(kind: Kind, frame: usize) -> (char, char) {
    let eye = if frame % 4 == 3 { '▄' } else { '█' };
    match kind {
        Kind::Info => (eye, '.'),
        Kind::Success => (eye, 'w'),
        Kind::Warn => ('O', 'o'),
        Kind::Error => (';', '~'),
    }
}

// The design's per-kind animations, dimension-preserving so nothing reflows:
// success hops using the reserved top row; error shakes inside a reserved
// side column; the rest stay still.
fn animate_art(mut art: Vec<String>, kind: Kind, frame: usize) -> Vec<String> {
    match kind {
        Kind::Success => {
            if frame % 2 == 1 {
                art.remove(0);
                let w = art.last().map(|l| l.chars().count()).unwrap_or(0);
                art.push(" ".repeat(w));
            }
        }
        Kind::Error => {
            let left = frame % 2 == 0;
            for l in art.iter_mut() {
                if left {
                    l.insert(0, ' ');
                } else {
                    l.push(' ');
                }
            }
        }
        _ => {}
    }
    art
}

fn animate_tiny(face: String, kind: Kind, frame: usize) -> String {
    if kind == Kind::Error {
        if frame % 2 == 0 { format!(" {face}") } else { format!("{face} ") }
    } else {
        face
    }
}

// As in the design's progress row: task name, a long green bar, green percent.
pub fn progress_line(from: &str, pct: u8) -> Line {
    let cells = 20usize;
    let filled = (pct as usize * cells) / 100;
    let name = if from.is_empty() { i18n::PROGRESS_DEFAULT } else { from };
    vec![
        seg(format!("{name} "), None),
        seg("█".repeat(filled), Some(Color::Green)),
        seg("░".repeat(cells - filled), Some(Color::DarkGrey)),
        seg(format!(" {pct}%"), Some(Color::Green)),
    ]
}

fn wrap(text: &str, w: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > w {
            lines.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    lines
}

fn mood_line(pet: &Pet) -> Line {
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

const GRASS: &str = "▁▂▁▁▃▁▂▁▁▁▂▁▃▁▁▂▁▁▁▂▁▃▁▂▁▁";

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
        content.push(center_in(&tinted(i18n::ZEN_MODE, Color::DarkGrey), w));
        return content;
    }

    let mut right: Vec<Line> = panel(i18n::PANEL_STATUS, &stat_bars(pet, false), right_w);
    if tastes {
        let (likes, hates) = i18n::species_tastes(pet.species);
        right.extend(panel(
            i18n::PANEL_MOOD,
            &[
                plain(i18n::species_trait(pet.species)),
                tinted(format!("{}: {likes}", i18n::LIKES), Color::DarkGrey),
                tinted(format!("{}: {hates}", i18n::HATES), Color::DarkGrey),
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
            events.push(tinted(i18n::LOG_EMPTY, Color::DarkGrey));
        }
        while events.len() < event_rows {
            events.push(Vec::new());
        }
        content.extend(panel(i18n::PANEL_EVENTS, &events, w));
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
            content.push(tinted(i18n::ZEN_MODE, Color::DarkGrey));
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
        format!(" {} {}{}", i18n::LEVEL_SHORT, pet.level, if pet.zen { " · zen" } else { "" }),
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

fn build_home(pet: &Pet, frame: usize, view: &HomeView, iw: usize, ih: usize) -> Vec<Line> {
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
    draw_screen(out, &content, &i18n::FOOTER_HOME)
}

fn food_effects(food: &crate::pet::Food) -> Line {
    let mut l: Line = Vec::new();
    for (delta, label) in [
        (food.hunger, i18n::STAT_LABELS[0]),
        (food.happiness, i18n::STAT_LABELS[1]),
        (food.energy, i18n::STAT_LABELS[2]),
        (food.hygiene, i18n::STAT_LABELS[3]),
    ] {
        if delta != 0 {
            let c = if delta > 0 { Color::Green } else { Color::Red };
            l.push(seg(format!("{delta:+} {label}  "), Some(c)));
        }
    }
    l
}

// Food icons from the design's menu (bowl, fish, cupcake, teacup), redrawn as
// single-width glyph triplets — emoji take two cells and would shear the
// alignment. Index-aligned with pet::FOODS.
const FOOD_ICONS: [&str; 4] = [r"\∴/", "<><", "(@)", r"\_/"];

// Index-aligned with app::Action and i18n::ACTION_LABELS.
pub const ACTION_GLYPHS: [&str; 9] = [r"\∴/", "(o)", "z Z", "oOo", "1v1", "(!)", "(*)", "-_-", "<=>"];

// A window of `len` chars starting at `start`, preserving segment colors.
fn line_slice(l: &Line, start: usize, len: usize) -> Line {
    let mut out: Line = Vec::new();
    let mut pos = 0usize;
    for (s, fg, bg) in l {
        let seg_start = pos;
        pos += s.chars().count();
        let from = start.max(seg_start);
        let to = (start + len).min(pos);
        if to > from {
            let t: String = s.chars().skip(from - seg_start).take(to - from).collect();
            out.push((t, *fg, *bg));
        }
    }
    out
}

fn dim(lines: &[Line]) -> Vec<Line> {
    lines
        .iter()
        .map(|l| l.iter().map(|(s, ..)| (s.clone(), Some(Color::DarkGrey), None)).collect())
        .collect()
}

// Splices a modal block over the center of a backdrop, per the design's
// overlay: the screen behind stays visible, dimmed. Falls back to the modal
// alone when the backdrop is too small to hold it.
fn overlay(base: Vec<Line>, over: &[Line]) -> Vec<Line> {
    let mut base = pad_block(base);
    let bw = base.iter().map(line_w).max().unwrap_or(0);
    let ow = over.iter().map(line_w).max().unwrap_or(0);
    if ow > bw || over.len() > base.len() {
        return over.to_vec();
    }
    let top = (base.len() - over.len()) / 2;
    let left = (bw - ow) / 2;
    for (i, o) in over.iter().enumerate() {
        let row = &base[top + i];
        let mut composed = line_slice(row, 0, left);
        composed.extend(clip_pad(o, ow));
        composed.extend(line_slice(row, left + ow, bw - left - ow));
        base[top + i] = composed;
    }
    base
}

// The actions overlay from the controls redesign: a numbered modal list
// floating over the dimmed home screen.
pub fn draw_actions(
    out: &mut impl Write,
    pet: &Pet,
    frame: usize,
    view: &HomeView,
    items: &[usize],
    sel: usize,
) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let w = iw.min(38);
    let mut body: Vec<Line> = Vec::new();
    for (i, &action) in items.iter().enumerate() {
        let selected = i == sel;
        body.push(vec![
            seg(if selected { "▸ " } else { "  " }, Some(Color::Cyan)),
            seg(format!("{} ", i + 1), Some(Color::Cyan)),
            seg(format!("{:<5}", ACTION_GLYPHS[action]), Some(Color::DarkGrey)),
            seg(i18n::ACTION_LABELS[action], if selected { Some(Color::Cyan) } else { None }),
        ]);
    }
    let modal = boxed(Some((i18n::ACTIONS_TITLE, Color::Magenta)), Color::Magenta, &body, w);
    let backdrop = dim(&build_home(pet, frame, view, iw, ih));
    let content = overlay(backdrop, &modal);
    draw_screen(out, &content, &i18n::FOOTER_ACTIONS)
}

// The design's menu: a bordered modal titled "cardápio", ▸ marking the
// selected row, an icon per food, effects colored by sign — floating over
// the dimmed home screen like every modal.
pub fn draw_menu(out: &mut impl Write, pet: &Pet, frame: usize, view: &HomeView, sel: usize) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let w = iw.min(60);
    let mut body: Vec<Line> = Vec::new();
    for (i, food) in FOODS.iter().enumerate() {
        let selected = i == sel;
        let mut l: Line = vec![
            seg(if selected { "▸ " } else { "  " }, Some(Color::Cyan)),
            seg(format!("{} ", FOOD_ICONS[i]), Some(Color::DarkGrey)),
            seg(format!("{:<17}", i18n::FOOD_NAMES[i]), if selected { Some(Color::Cyan) } else { None }),
        ];
        l.extend(food_effects(food));
        body.push(l);
    }
    let modal = boxed(Some((i18n::MENU_TITLE, Color::Magenta)), Color::Magenta, &body, w);
    let content = overlay(dim(&build_home(pet, frame, view, iw, ih)), &modal);
    draw_screen(out, &content, &i18n::FOOTER_MENU)
}

// 3×5 pixel bitmaps for the big LCD clock, tty-clock style; each pixel
// renders as a double-width "██" block so digits read square on screen.
// Each row is 3 bits, most significant bit = left column.
const DIGIT_BITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

// Renders "24:59" as 5 rows of block art. Unknown chars are skipped.
pub fn big_time(text: &str) -> Vec<String> {
    let mut rows = vec![String::new(); 5];
    for ch in text.chars() {
        for (r, row) in rows.iter_mut().enumerate() {
            match ch {
                '0'..='9' => {
                    let bits = DIGIT_BITS[ch as usize - '0' as usize][r];
                    for c in [2u8, 1, 0] {
                        row.push_str(if bits >> c & 1 == 1 { "██" } else { "  " });
                    }
                    row.push(' ');
                }
                ':' => row.push_str(if r == 1 || r == 3 { "██ " } else { "   " }),
                _ => {}
            }
        }
    }
    rows
}

// What draw_pomo shows for a running cycle.
pub struct PomoRun {
    pub label: &'static str, // "foco" / "pausa"
    pub focus: bool,
    pub frac: u8, // elapsed % of the current phase
    pub cycle: u32,
}

fn preset_rows(sel: usize) -> Vec<Line> {
    i18n::POMO_PRESET_LABELS
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
        seg(format!("  {} {}", i18n::POMO_CYCLE, run.cycle), Some(Color::DarkGrey)),
    ]
}

fn task_rows(view: &HomeView, rows: usize) -> Vec<Line> {
    let mut tasks: Vec<Line> = view.progress.iter().take(rows).cloned().collect();
    if tasks.is_empty() {
        tasks.push(tinted(i18n::POMO_NO_TASKS, Color::DarkGrey));
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
    let title = run.map_or(i18n::POMO_TITLE, |r| r.label);
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
    while column.len() < clock_art.len() + 2 + i18n::POMO_PRESET_LABELS.len() {
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
            content.extend(panel(i18n::POMO_TASKS, &task_rows(view, 3), w));
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
    let chip: Line = vec![(format!(" {} ", i18n::POMO_TITLE), Some(Color::Cyan), Some(Color::DarkGrey))];
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
        if run.is_some() { &i18n::FOOTER_POMO_ACTIVE } else { &i18n::FOOTER_POMO_IDLE };
    draw_screen(out, &content, footers)
}

pub fn draw_game(out: &mut impl Write, pet: &Pet, frame: usize, view: &HomeView) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    let (iw, ih) = (cols.saturating_sub(2) as usize, rows.saturating_sub(2) as usize);
    let waiting = i18n::msg_game_waiting(&pet.name);
    let w = iw.min(waiting.chars().count().max(30) + 6);
    let body: Vec<Line> = vec![
        plain(waiting),
        Vec::new(),
        vec![
            chip("1"),
            seg(format!(" {}   ", i18n::HANDS[0]), None),
            chip("2"),
            seg(format!(" {}   ", i18n::HANDS[1]), None),
            chip("3"),
            seg(format!(" {}", i18n::HANDS[2]), None),
        ],
    ];
    let modal = boxed(Some((i18n::GAME_TITLE, Color::Magenta)), Color::Magenta, &body, w);
    let content = overlay(dim(&build_home(pet, frame, view, iw, ih)), &modal);
    draw_screen(out, &content, &i18n::FOOTER_GAME)
}

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
}

const BUBBLE_TEXT_ROWS: usize = 4; // fixed: message length must not resize the layout
const QUEUE_ROWS: usize = 2;
const OPTION_ROWS: usize = 3; // fixed: option count must not resize the layout

// Fixed-width countdown so the ticking never reflows the line.
fn countdown_seg(expires_in: u64) -> Seg {
    let color = if expires_in <= 10 { Color::Red } else { Color::Yellow };
    (format!("{} {:>3}s", i18n::EXPIRES_LABEL, expires_in.min(999)), Some(color), None)
}

// Truncates to `w` with a visible … instead of clip_pad's silent cut.
fn ellipsize(line: Line, w: usize) -> Line {
    if line_w(&line) <= w {
        return line;
    }
    let mut out = clip_pad(&line, w.saturating_sub(1));
    out.push(seg("…", Some(Color::DarkGrey)));
    out
}

// wrap() capped at `rows` lines; overflow ends the last visible row with …
fn wrapped_text(text: &str, w: usize, rows: usize) -> Vec<Line> {
    let mut wrapped = wrap(text, w);
    if wrapped.len() > rows.max(1) {
        wrapped.truncate(rows.max(1));
        let last = wrapped.last_mut().unwrap();
        let keep: String = last.chars().take(w.saturating_sub(1)).collect();
        *last = format!("{keep}…");
    }
    wrapped.into_iter().map(plain).collect()
}

// The typed-answer field, right-anchored so the caret stays visible while
// the text grows past the width.
fn input_row(buf: &str, w: usize) -> Line {
    let inner = w.saturating_sub(3); // "> " + caret
    let shown: String = match buf.chars().count() > inner {
        true => buf.chars().skip(buf.chars().count() - inner).collect(),
        false => buf.to_string(),
    };
    vec![seg("> ", Some(Color::DarkGrey)), seg(shown, None), seg("_", Some(Color::Cyan))]
}

// Rows an ask occupies below its text: the typing field replaces the options
// while it is open (same slot count either way — no reflow).
fn answer_rows(m: &AssistantMsg, w: usize) -> Vec<Line> {
    match m.input {
        Some(buf) => {
            let mut rows = vec![input_row(buf, w)];
            while rows.len() < OPTION_ROWS {
                rows.push(Vec::new());
            }
            rows
        }
        None => option_rows(&option_labels(m.options.unwrap_or_default(), m.input_ok), w),
    }
}

// The choices as shown: the fixed options plus, when free text is accepted,
// one more numbered entry for it — the "Other" of the harness prompts. It
// only fits while there is a key left (1-9).
pub fn option_labels(options: &[String], input_ok: bool) -> Vec<String> {
    let mut labels: Vec<String> = options.iter().take(9).cloned().collect();
    if input_ok && labels.len() < 9 {
        labels.push(i18n::OPTION_WRITE.to_string());
    }
    labels
}

// One option per row in OPTION_ROWS fixed slots (blank-padded, so option
// count never resizes the layout). Options past the last slot pack into it;
// anything wider than `w` clips with a trailing …
fn option_rows(options: &[String], w: usize) -> Vec<Line> {
    let mut rows: Vec<Line> = Vec::new();
    for (i, o) in options.iter().enumerate().take(9) {
        let mut item: Line = vec![chip(&(i + 1).to_string()), seg(format!(" {o}"), None)];
        match rows.len() < OPTION_ROWS {
            true => rows.push(item),
            false => {
                let last = rows.last_mut().unwrap();
                last.push(seg("  ", None));
                last.append(&mut item);
            }
        }
    }
    while rows.len() < OPTION_ROWS {
        rows.push(Vec::new());
    }
    rows.into_iter().map(|r| ellipsize(r, w)).collect()
}

// The design's speech bubble: an untitled box in the kind's color with a tail
// pointing at the pet, the message inside, a `de · tipo · hora` meta row and
// OPTION_ROWS option slots. Always the same height for every message shape.
fn bubble_panel(msg: Option<&AssistantMsg>, clock_text: &str, w: usize) -> Vec<Line> {
    const BODY_ROWS: usize = BUBBLE_TEXT_ROWS + 2 + OPTION_ROWS; // text + blank + meta + options
    let inner = w.saturating_sub(4);
    let Some(m) = msg else {
        let mut body: Vec<Line> = vec![tinted(i18n::NO_MESSAGES, Color::DarkGrey)];
        while body.len() < BODY_ROWS {
            body.push(Vec::new());
        }
        return boxed(None, Color::DarkGrey, &body, w);
    };

    let color = kind_color(m.kind);
    let mut body: Vec<Line> = Vec::new();
    if m.options.is_some() && !m.from.is_empty() {
        body.push(tinted(format!("{} {}:", m.from, i18n::ASKS_VERB), Color::DarkGrey));
    }
    let text_rows = BUBBLE_TEXT_ROWS - body.len();
    body.extend(wrapped_text(m.text, inner, text_rows));
    while body.len() < BUBBLE_TEXT_ROWS {
        body.push(Vec::new());
    }
    body.push(Vec::new());
    let mut meta: Line = Vec::new();
    if !m.from.is_empty() {
        meta.push(seg(format!("{}: ", i18n::FROM_LABEL), Some(Color::DarkGrey)));
        meta.push(seg(m.from, Some(color)));
        meta.push(seg("   ", None));
    }
    meta.push(seg(format!("{}: {}", i18n::TYPE_LABEL, m.kind_label), Some(Color::DarkGrey)));
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
        Some(m) if m.input.is_some() => &i18n::FOOTER_INPUT,
        Some(m) if m.options.is_some() => &i18n::FOOTER_ASK,
        _ => &i18n::FOOTER_ASSISTANT,
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
            right.extend(panel(&format!("{} ({queue_len})", i18n::PANEL_QUEUE), &queue_body, right_w));

            // Design: identity on the left, the "modo assistente" chip on the right.
            let (mut header, _) = header_parts(pet, view);
            let mut chip: Line = timer_segs(view);
            if !chip.is_empty() {
                chip.push(seg("   ", None));
            }
            chip.push((format!(" {} ", i18n::ASSISTANT_TAG), Some(Color::Cyan), Some(Color::DarkGrey)));
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
                    first.push(seg(format!("{} {}:", m.from, i18n::ASKS_VERB), Some(Color::DarkGrey)));
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
                    last.push(seg(format!("{}: ", i18n::FROM_LABEL), Some(Color::DarkGrey)));
                    last.push(seg(m.from, Some(color)));
                    last.push(seg("   ", None));
                }
                last.push(seg(format!("{}: {}", i18n::TYPE_LABEL, m.kind_label), Some(Color::DarkGrey)));
                body.push(last);
            }
            None => {
                body.push(tinted(i18n::NO_MESSAGES, Color::DarkGrey));
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
            c.push(tinted(format!("{} ({queue_len})", i18n::PANEL_QUEUE), Color::DarkGrey));
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
            rows.push(tinted(i18n::NO_MESSAGES, Color::DarkGrey));
        }
        rows.truncate(ih.saturating_sub(1).max(1));
        content = pad_block(rows);
    }
    content
}

pub fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
}

pub struct Clock {
    text: String,
    hour: u8,
    fetched: Instant,
}

impl Clock {
    pub fn new() -> Self {
        let (text, hour) = fetch_clock();
        Clock { text, hour, fetched: Instant::now() }
    }

    pub fn get(&mut self) -> (String, u8) {
        if self.fetched.elapsed() > Duration::from_secs(20) {
            let (text, hour) = fetch_clock();
            self.text = text;
            self.hour = hour;
            self.fetched = Instant::now();
        }
        (self.text.clone(), self.hour)
    }
}

// ponytail: local time via the `date` binary — std has no timezone support
// and a chrono dependency is not worth one HH:MM string. Cached for 20s.
fn fetch_clock() -> (String, u8) {
    std::process::Command::new("date")
        .arg("+%H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 5)
        .map(|s| {
            let hour = s[..2].parse().unwrap_or(12);
            (s, hour)
        })
        .unwrap_or_else(|| ("--:--".to_string(), 12))
}

#[cfg(test)]
mod tests {
    use super::*;

    static EMPTY_LOG: VecDeque<Line> = VecDeque::new();

    fn view_of(log: &VecDeque<Line>) -> HomeView<'_> {
        HomeView { log, clock_text: "12:00", hour: 12, timer: None, progress: Vec::new() }
    }

    fn sample_ask<'a>(text: &'a str, options: &'a [String], expires_in: Option<u64>) -> AssistantMsg<'a> {
        AssistantMsg {
            text,
            from: "claude",
            kind: Kind::Info,
            kind_label: "info",
            options: Some(options),
            expires_in,
            input: None,
            input_ok: false,
        }
    }

    #[test]
    fn free_text_is_listed_as_the_last_numbered_option() {
        let opts: Vec<String> = vec!["a".into(), "b".into()];
        assert_eq!(option_labels(&opts, false), opts);
        let with = option_labels(&opts, true);
        assert_eq!(with.len(), 3);
        assert_eq!(with[2], i18n::OPTION_WRITE);
        // no free key left (9 options) → no extra entry, `t` still opens it
        let nine: Vec<String> = (0..9).map(|i| i.to_string()).collect();
        assert_eq!(option_labels(&nine, true).len(), 9);
        // it renders in the card
        let pet = named_pet();
        let mut m = sample_ask("qual?", &opts, None);
        m.input_ok = true;
        let c = build_assistant(&pet, 0, Some(&m), &[], 0, &view_of(&EMPTY_LOG), 96, 24);
        let text: String = c.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
        assert!(text.contains(i18n::OPTION_WRITE), "extra option missing: {text}");
    }

    #[test]
    fn input_row_keeps_the_caret_visible_as_text_grows() {
        let short = input_row("oi", 20);
        assert_eq!(short.iter().map(|(s, ..)| s.clone()).collect::<String>(), "> oi_");
        // longer than the field: tail is shown, caret still last
        let long = input_row(&"a".repeat(50), 20);
        let text: String = long.iter().map(|(s, ..)| s.clone()).collect();
        assert!(text.starts_with("> ") && text.ends_with('_'));
        assert_eq!(text.chars().count(), 20);
    }

    #[test]
    fn typing_replaces_the_options_without_resizing_the_card() {
        let pet = named_pet();
        let options: Vec<String> = vec!["sim".into(), "não".into()];
        for (iw, ih) in [(96, 24), (80, 18), (50, 16), (26, 8)] {
            let idle = sample_ask("responde?", &options, None);
            let mut typing = sample_ask("responde?", &options, None);
            typing.input = Some("uma resposta escrita");
            let a = build_assistant(&pet, 0, Some(&idle), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
            let b = build_assistant(&pet, 0, Some(&typing), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
            assert_eq!(a.len(), b.len(), "card resized while typing at {iw}x{ih}");
            let text: String = b.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
            assert!(text.contains("resposta escrita"), "typed text missing at {iw}x{ih}");
        }
    }

    #[test]
    fn text_only_ask_has_no_options_but_still_fits() {
        let pet = named_pet();
        let empty: Vec<String> = Vec::new();
        let mut m = sample_ask("o que você acha?", &empty, Some(30));
        m.input = Some("porque sim");
        for (iw, ih) in [(96, 24), (60, 12), (26, 8)] {
            let c = build_assistant(&pet, 0, Some(&m), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
            assert!(c.len() <= ih.saturating_sub(1).max(1), "overflow at {iw}x{ih}");
            let text: String = c.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
            assert!(text.contains("porque sim"), "typed text missing at {iw}x{ih}");
        }
    }

    #[test]
    fn build_assistant_fits_any_terminal_size() {
        let pet = named_pet();
        let long_text = "claude quer executar: npm run build && rm -rf dist && cp x y ".repeat(5);
        let options: Vec<String> = vec![
            "permitir".into(),
            "Sim, e não pergunte de novo nesta sessão inteira por favor".into(),
            "negar".into(),
            "decidir no claude".into(),
        ];
        let queue = vec!["ci: build ok".to_string()];
        for iw in [10, 20, 26, 30, 45, 60, 72, 80, 96, 120] {
            for ih in [1, 3, 5, 6, 8, 12, 16, 20, 24, 28, 40] {
                for msg in [
                    None,
                    Some(sample_ask(&long_text, &options, Some(59))),
                    Some(AssistantMsg {
                        text: "oi",
                        from: "ci",
                        kind: Kind::Success,
                        kind_label: "sucesso",
                        options: None,
                        expires_in: None,
                        input: None,
                        input_ok: false,
                    }),
                ] {
                    let c = build_assistant(&pet, 0, msg.as_ref(), &queue, 1, &view_of(&EMPTY_LOG), iw, ih);
                    assert!(
                        c.len() <= ih.saturating_sub(1).max(1),
                        "overflow at {iw}x{ih}: {} lines",
                        c.len()
                    );
                }
            }
        }
    }

    #[test]
    fn option_count_never_resizes_the_ask_bubble() {
        let pet = named_pet();
        for (iw, ih) in [(96, 24), (80, 18), (50, 16), (26, 8)] {
            let mut baseline: Option<usize> = None;
            for n in [1usize, 3, 9] {
                let options: Vec<String> = (0..n).map(|i| format!("opção {i}")).collect();
                let msg = sample_ask("posso?", &options, None);
                let c = build_assistant(&pet, 0, Some(&msg), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
                match &baseline {
                    None => baseline = Some(c.len()),
                    Some(b) => assert_eq!(*b, c.len(), "bubble resized at {iw}x{ih} with {n} options"),
                }
            }
        }
    }

    #[test]
    fn option_rows_are_one_per_slot_and_clip_with_ellipsis() {
        let opts: Vec<String> = vec!["curta".into(), "uma opção comprida demais para caber".into()];
        let rows = option_rows(&opts, 16);
        assert_eq!(rows.len(), OPTION_ROWS);
        assert!(rows[0].iter().any(|(s, ..)| s.contains("curta")));
        let second: String = rows[1].iter().map(|(s, ..)| s.clone()).collect();
        assert!(second.ends_with('…'), "clipped option should end in …: {second:?}");
        assert_eq!(line_w(&rows[1]), 16);
        assert_eq!(line_w(&rows[2]), 0); // empty slot stays reserved
        // options past the last slot pack into it
        let many: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect();
        let rows = option_rows(&many, 60);
        assert_eq!(rows.len(), OPTION_ROWS);
        let last: String = rows[2].iter().map(|(s, ..)| s.clone()).collect();
        assert!(last.contains('c') && last.contains('d') && last.contains('e'), "{last:?}");
    }

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

    #[test]
    fn wrapped_text_marks_truncation_with_ellipsis() {
        let full = wrapped_text("uma frase curta", 40, 3);
        assert_eq!(full.len(), 1);
        let cut = wrapped_text("muitas palavras que não cabem em uma linha só de jeito nenhum", 12, 2);
        assert_eq!(cut.len(), 2);
        let last: String = cut[1].iter().map(|(s, ..)| s.clone()).collect();
        assert!(last.ends_with('…'), "{last:?}");
    }

    // Every joined line must have the SAME width: draw_screen centers lines
    // independently, so any variation shears the blocks apart.
    #[test]
    fn beside_joins_blocks_at_uniform_width() {
        let left = vec![plain("aa"), plain("a")];
        let right = vec![plain("bb"), plain("b"), plain("c")];
        let joined = beside(&left, &right, 2);
        assert_eq!(joined.len(), 3);
        assert!(joined.iter().all(|l| line_w(l) == 2 + 2 + 2));
    }

    #[test]
    fn pad_block_makes_widths_uniform() {
        let block = pad_block(vec![plain("abc"), plain("a")]);
        assert!(block.iter().all(|l| line_w(l) == 3));
    }

    #[test]
    fn panel_has_uniform_width_and_borders() {
        let p = panel("status", &[plain("hi"), plain("a much longer line that overflows")], 20);
        assert_eq!(p.len(), 4);
        assert!(p.iter().all(|l| line_w(l) == 20));
        assert_eq!(p[0][0].0, "┌─ ");
        assert_eq!(p[0][1].0, "status");
        assert!(p[3][0].0.starts_with('└'));
    }

    #[test]
    fn center_in_clips_or_centers() {
        assert_eq!(line_w(&center_in(&plain("ab"), 6)), 6);
        assert_eq!(line_w(&center_in(&plain("abcdefgh"), 4)), 4);
    }

    fn named_pet() -> Pet {
        Pet { name: "rex".into(), ..Pet::default() }
    }

    // The content must fit the inner height (minus the footer row) at EVERY
    // terminal size — height responsiveness is exactly this invariant.
    #[test]
    fn build_home_fits_any_terminal_size() {
        let pet = named_pet();
        let log = VecDeque::new();
        for iw in [10, 20, 30, 45, 60, 72, 80, 96, 120] {
            for ih in [1, 3, 5, 8, 12, 16, 20, 24, 28, 40] {
                let c = build_home(&pet, 0, &view_of(&log), iw, ih);
                assert!(
                    c.len() <= ih.saturating_sub(1).max(1),
                    "overflow at {iw}x{ih}: {} lines",
                    c.len()
                );
            }
        }
    }

    #[test]
    fn tall_wide_terminal_gets_full_panel_layout() {
        let pet = named_pet();
        let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 96, 30);
        let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
        assert!(text.contains("┌─ rex"));
        assert!(text.contains("┌─ status"));
        assert!(text.contains("┌─ eventos"));
    }

    #[test]
    fn short_wide_terminal_degrades_but_keeps_panels_when_possible() {
        let pet = named_pet();
        let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 96, 16);
        let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
        assert!(text.contains("┌─ rex"), "should still use the panel layout at 96x16");
        assert!(!text.contains("┌─ eventos") || c.len() <= 15);
    }

    // A tall terminal must be USED, not just centered into: the leftover
    // height flows into the pet scene (sky) and extra log entries.
    #[test]
    fn tall_terminal_fills_available_height() {
        let pet = named_pet();
        let mut log = VecDeque::new();
        for i in 0..8 {
            log.push_back(plain(format!("event {i}")));
        }
        let avail = 39; // 96x40 terminal
        let c = build_home(&pet, 0, &view_of(&log), 96, 40);
        assert!(c.len() >= avail - 4, "only {} of {avail} lines used", c.len());
        assert!(c.len() <= avail);
    }

    // The layout skeleton must not move as the log fills: same height and
    // same row for every panel with 0, 1 or 12 events, at several sizes.
    #[test]
    fn event_count_never_resizes_the_layout() {
        let pet = named_pet();
        let row_of = |c: &[Line], needle: &str| {
            c.iter().position(|l| l.iter().any(|(s, ..)| s.contains(needle)))
        };
        for (iw, ih) in [(96, 40), (96, 30), (96, 24), (80, 18), (50, 16), (30, 12)] {
            let mut baseline: Option<(usize, Option<usize>, Option<usize>)> = None;
            for n in [0usize, 1, 12] {
                let mut log = VecDeque::new();
                for i in 0..n {
                    log.push_back(plain(format!("event {i}")));
                }
                let c = build_home(&pet, 0, &view_of(&log), iw, ih);
                let shape = (c.len(), row_of(&c, "eventos"), row_of(&c, "▄█▄"));
                match &baseline {
                    None => baseline = Some(shape),
                    Some(b) => assert_eq!(*b, shape, "layout moved at {iw}x{ih} with {n} events"),
                }
            }
        }
    }

    #[test]
    fn line_slice_cuts_across_segments() {
        let l: Line = vec![seg("abc", None), seg("def", Some(Color::Red))];
        assert_eq!(line_slice(&l, 1, 4).iter().map(|(s, ..)| s.as_str()).collect::<String>(), "bcde");
        assert_eq!(line_w(&line_slice(&l, 0, 6)), 6);
        assert_eq!(line_w(&line_slice(&l, 4, 10)), 2);
    }

    // The modal must sit centered over the backdrop with the backdrop intact
    // around it — total dimensions unchanged.
    #[test]
    fn overlay_centers_modal_and_keeps_backdrop_dimensions() {
        let base: Vec<Line> = (0..9).map(|_| plain("##########")).collect();
        let modal: Vec<Line> = (0..3).map(|_| plain("XXXX")).collect();
        let out = overlay(base, &modal);
        assert_eq!(out.len(), 9);
        assert!(out.iter().all(|l| line_w(l) == 10));
        let mid: String = out[4].iter().map(|(s, ..)| s.as_str()).collect();
        assert_eq!(mid, "###XXXX###");
        let top: String = out[0].iter().map(|(s, ..)| s.as_str()).collect();
        assert_eq!(top, "##########");
    }

    #[test]
    fn overlay_too_big_falls_back_to_modal_alone() {
        let base: Vec<Line> = vec![plain("##")];
        let modal: Vec<Line> = vec![plain("XXXX"), plain("XXXX")];
        assert_eq!(overlay(base, &modal).len(), 2);
    }

    #[test]
    fn kind_faces_are_distinct_and_warn_never_blinks() {
        let kinds = [Kind::Info, Kind::Success, Kind::Warn, Kind::Error];
        for (i, a) in kinds.iter().enumerate() {
            for b in kinds.iter().skip(i + 1) {
                assert_ne!(kind_face(*a, 0), kind_face(*b, 0), "{a:?} vs {b:?}");
            }
        }
        assert_eq!(kind_face(Kind::Warn, 3), kind_face(Kind::Warn, 0));
        assert_ne!(kind_face(Kind::Info, 3).0, kind_face(Kind::Info, 0).0); // blinks
    }

    // Hop and shake must not change the art's footprint on any frame.
    #[test]
    fn kind_animations_preserve_dimensions() {
        use crate::species::{render_art_face, Species};
        for kind in [Kind::Info, Kind::Success, Kind::Warn, Kind::Error] {
            let mut shapes = Vec::new();
            for frame in 0..4 {
                let (eye, mouth) = kind_face(kind, frame);
                let art = animate_art(render_art_face(Species::Dragon, ArtSize::Large, eye, mouth), kind, frame);
                let w = art[0].chars().count();
                assert!(art.iter().all(|l| l.chars().count() == w), "{kind:?} misaligned at frame {frame}");
                shapes.push((art.len(), w));
            }
            assert!(shapes.windows(2).all(|p| p[0] == p[1]), "{kind:?} footprint changed across frames");
        }
    }

    #[test]
    fn tiny_terminal_falls_back_to_face() {
        let pet = named_pet();
        let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 24, 4);
        let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
        assert!(text.contains("(=^"), "tiny face expected at 24x4");
    }

    #[test]
    fn line_w_sums_segments_by_chars_not_bytes() {
        let l: Line = vec![seg("██", None), seg("ab", None)];
        assert_eq!(line_w(&l), 4);
    }

    #[test]
    fn stat_bars_cover_all_stats_plus_xp() {
        let pet = Pet::default();
        assert_eq!(stat_bars(&pet, false).len(), 5);
    }

    #[test]
    fn big_time_renders_uniform_rows_and_distinct_digits() {
        let art = big_time("25:09");
        assert_eq!(art.len(), 5);
        let w = art[0].chars().count();
        assert_eq!(w, 4 * 7 + 3); // 4 digits + colon
        assert!(art.iter().all(|l| l.chars().count() == w));
        for a in 0..10u8 {
            for b in (a + 1)..10 {
                assert_ne!(big_time(&a.to_string()), big_time(&b.to_string()), "{a} vs {b}");
            }
        }
        assert!(big_time("x").iter().all(|l| l.is_empty())); // unknown chars skipped
    }

    // Same invariant as build_home: the pomodoro screen must fit the inner
    // height at every terminal size, running or idle.
    #[test]
    fn build_pomo_fits_any_terminal_size() {
        let pet = named_pet();
        let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
        for iw in [10, 20, 30, 45, 60, 72, 80, 96, 120] {
            for ih in [1, 3, 5, 8, 12, 16, 20, 24, 28, 40] {
                for r in [None, Some(&run)] {
                    let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, iw, ih);
                    assert!(
                        c.len() <= ih.saturating_sub(1).max(1),
                        "overflow at {iw}x{ih} (run={}): {} lines",
                        r.is_some(),
                        c.len()
                    );
                }
            }
        }
    }

    // The pet must be on the pomodoro screen at EVERY size that has room for
    // more than the single status line: full art, or the tiny face fallback.
    #[test]
    fn pomo_screen_always_shows_the_pet() {
        let pet = named_pet();
        for (iw, ih) in [(96, 30), (96, 14), (60, 12), (40, 10), (30, 5)] {
            let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, iw, ih);
            let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
            assert!(text.contains("▄█▄") || text.contains("(=^"), "no pet at {iw}x{ih}");
        }
    }

    // Compact tiers must form ONE block: draw_screen centers lines
    // independently, so any width variation would give the mascot, clock and
    // presets each their own left edge.
    #[test]
    fn pomo_compact_tier_keeps_a_single_left_edge() {
        let pet = named_pet();
        let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
        for (iw, ih) in [(71, 20), (60, 14), (44, 12), (36, 10)] {
            for r in [None, Some(&run)] {
                let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, iw, ih);
                let w = c.iter().map(line_w).max().unwrap_or(0);
                assert!(
                    c.iter().all(|l| line_w(l) == w),
                    "ragged block at {iw}x{ih} (run={})",
                    r.is_some()
                );
            }
        }
    }

    // (row, col) of the clock's top-left edge: the first run of 6 '█' (the
    // top row of the first digit of "25:00").
    fn clock_pos(c: &[Line]) -> Option<(usize, usize)> {
        for (y, l) in c.iter().enumerate() {
            let chars: Vec<char> = l.iter().flat_map(|(s, ..)| s.chars()).collect();
            if let Some(x) = chars.windows(6).position(|w| w.iter().all(|&ch| ch == '█')) {
                return Some((y, x));
            }
        }
        None
    }

    fn clock_x(c: &[Line]) -> Option<usize> {
        clock_pos(c).map(|(_, x)| x)
    }

    // The clock must sit at the exact same x whatever the state (idle,
    // running, with tasks) and whatever art rung the height picks — and that
    // x must be the horizontal center.
    #[test]
    fn pomo_clock_is_pinned_to_the_horizontal_center() {
        let pet = named_pet();
        let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
        let tasks = vec![progress_line("build", 40)];
        let states: [(Option<&PomoRun>, &Vec<Line>); 3] =
            [(None, &Vec::new()), (Some(&run), &Vec::new()), (Some(&run), &tasks)];
        let clock_w = 31; // big "25:00"
        // Full tier, several terminal widths (wider than the 96-col panel cap
        // too) and both art rungs (tall → large art, short → small art): the
        // pet+clock group must sit with EQUAL screen margins, and must not
        // move — in x OR y — when the state flips.
        for iw in [96, 100, 110] {
            for ih in [30, 26, 21] {
                let mut pos_seen = None;
                for (r, progress) in &states {
                    let view = HomeView {
                        log: &EMPTY_LOG,
                        clock_text: "12:00",
                        hour: 12,
                        timer: None,
                        progress: (*progress).clone(),
                    };
                    let c = build_pomo(&pet, 0, &view, "25:00", *r, 0, iw, ih);
                    let pos = clock_pos(&c).unwrap();
                    assert_eq!(*pos_seen.get_or_insert(pos), pos, "clock moved at {iw}x{ih}");
                    // screen coords: add draw_screen's centering pad
                    let lw = c.iter().map(line_w).max().unwrap();
                    let pad = (iw - lw) / 2;
                    // the pet's left edge on screen, via the grass row (it
                    // spans the pet block from its column 0)
                    let left = c
                        .iter()
                        .find_map(|l| {
                            let chars: Vec<char> = l.iter().flat_map(|(s, ..)| s.chars()).collect();
                            chars.iter().position(|&ch| ch == '▁').map(|i| pad + i)
                        })
                        .unwrap();
                    let right = iw - (pad + pos.1 + clock_w);
                    assert!(
                        (left as i64 - right as i64).abs() <= 1,
                        "unbalanced at {iw}x{ih}: left {left}, right {right}"
                    );
                }
            }
        }
        // With the tall art, the clock group must share the pet's vertical
        // center (offset down), not hug the top of the panel.
        let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, 96, 30);
        let (y, _) = clock_pos(&c).unwrap();
        assert!(y > 4, "clock is top-aligned against the pet: row {y}");
        // compact tier: the block is exactly clock-wide, so draw_screen's
        // centering lands the clock itself on the center.
        for (iw, ih) in [(70, 16), (50, 14), (40, 12)] {
            for (r, progress) in &states {
                let view = HomeView {
                    log: &EMPTY_LOG,
                    clock_text: "12:00",
                    hour: 12,
                    timer: None,
                    progress: (*progress).clone(),
                };
                let c = build_pomo(&pet, 0, &view, "25:00", *r, 0, iw, ih);
                let w = c.iter().map(line_w).max().unwrap();
                assert_eq!(w, clock_w, "block wider than the clock at {iw}x{ih}");
                assert_eq!(clock_x(&c), Some(0));
            }
        }
    }

    // The compact tier's tiny face must not sit glued to the clock: the
    // column's breathing row provides one blank line between them.
    #[test]
    fn pomo_compact_keeps_air_between_face_and_clock() {
        let pet = named_pet();
        let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 1 };
        for r in [None, Some(&run)] {
            let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, 60, 16);
            let face_row =
                c.iter().position(|l| l.iter().any(|(s, ..)| s.contains("(=^"))).unwrap();
            let below: String = c[face_row + 1].iter().map(|(s, ..)| s.as_str()).collect();
            assert!(below.trim().is_empty(), "face glued to the clock: {below:?}");
        }
    }

    #[test]
    fn pomo_full_tier_shows_clock_beside_pet_and_tasks_panel() {
        let pet = named_pet();
        let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, 96, 30);
        let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
        assert!(text.contains("┌─ pomodoro"));
        assert!(text.contains("┌─ tarefas em andamento"));
        assert!(text.contains("25m foco"));
        assert!(text.contains("██")); // the big clock is there
    }

    #[test]
    fn clock_falls_back_gracefully() {
        let (text, hour) = fetch_clock();
        assert!(text == "--:--" || text.len() == 5);
        assert!(hour < 24);
    }
}
