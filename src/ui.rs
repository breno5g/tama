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

fn panel_titled(title: &str, title_color: Color, body: &[Line], w: usize) -> Vec<Line> {
    boxed(Some((title, title_color)), Color::DarkGrey, body, w)
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
    pub timer: Option<String>,
    pub progress: Option<Line>,
}

// Design: dimmed label, only the countdown in yellow.
fn timer_segs(view: &HomeView) -> Line {
    view.timer
        .as_ref()
        .map(|t| {
            vec![
                seg(format!("{} ", i18n::TIMER_LABEL), Some(Color::DarkGrey)),
                seg(t.clone(), Some(Color::Yellow)),
            ]
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
        let mut events: Vec<Line> = Vec::new();
        if let Some(p) = &view.progress {
            events.push(p.clone());
        }
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
        let ticker = view.progress.clone().or_else(|| view.log.back().cloned());
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
    let ticker = || view.progress.clone().or_else(|| view.log.back().cloned());

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
pub const ACTION_GLYPHS: [&str; 8] = [r"\∴/", "(o)", "z Z", "oOo", "1v1", "(!)", "-_-", "<=>"];

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
// selected row, an icon per food, effects colored by sign.
pub fn draw_menu(out: &mut impl Write, sel: usize) -> io::Result<()> {
    let (cols, _) = terminal::size()?;
    let iw = cols.saturating_sub(2) as usize;
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
    let content = panel_titled(i18n::MENU_TITLE, Color::Magenta, &body, w);
    draw_screen(out, &content, &i18n::FOOTER_MENU)
}

pub fn draw_game(out: &mut impl Write, pet: &Pet, frame: usize) -> io::Result<()> {
    let (_, rows) = terminal::size()?;
    let ih = rows.saturating_sub(2) as usize;
    let mut content: Vec<Line> = vec![tinted(i18n::GAME_TITLE, Color::Magenta), Vec::new()];
    if ih >= 14 {
        content.extend(render_art(pet.species, Mood::Happy, frame, ArtSize::Small).iter().map(|l| plain(l.clone())));
        content.push(Vec::new());
    }
    content.push(plain(i18n::msg_game_waiting(&pet.name)));
    draw_screen(out, &content, &i18n::FOOTER_GAME)
}

// What draw_assistant shows for the current message.
pub struct AssistantMsg<'a> {
    pub text: &'a str,
    pub from: &'a str,
    pub kind: Kind,
    pub kind_label: &'a str,
    pub options: Option<&'a [String]>,
}

const BUBBLE_TEXT_ROWS: usize = 4; // fixed: message length must not resize the layout
const QUEUE_ROWS: usize = 2;

// The design's speech bubble: an untitled box in the kind's color with a tail
// pointing at the pet, the message inside, and a `de · tipo · hora` meta row.
fn bubble_panel(msg: Option<&AssistantMsg>, clock_text: &str, w: usize) -> Vec<Line> {
    let inner = w.saturating_sub(4);
    let Some(m) = msg else {
        let mut body: Vec<Line> = vec![tinted(i18n::NO_MESSAGES, Color::DarkGrey)];
        while body.len() < BUBBLE_TEXT_ROWS + 3 {
            body.push(Vec::new());
        }
        return boxed(None, Color::DarkGrey, &body, w);
    };

    let color = kind_color(m.kind);
    let mut body: Vec<Line> = Vec::new();
    if m.options.is_some() && !m.from.is_empty() {
        body.push(tinted(format!("{} {}:", m.from, i18n::ASKS_VERB), Color::DarkGrey));
    }
    body.extend(wrap(m.text, inner).into_iter().take(BUBBLE_TEXT_ROWS - body.len()).map(plain));
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
    body.push(meta);
    let mut opts: Line = Vec::new();
    if let Some(options) = m.options {
        for (i, o) in options.iter().enumerate().take(9) {
            opts.push(chip(&(i + 1).to_string()));
            opts.push(seg(format!(" {o}   "), None));
        }
        opts.push(chip("esc"));
        opts.push(seg(format!(" {}", i18n::ESC_IGNORE), Some(Color::DarkGrey)));
    }
    body.push(opts);

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
    // Per-kind expression and animation; a calm happy face while idle.
    let face = msg
        .map(|m| kind_face(m.kind, frame))
        .unwrap_or_else(|| Mood::Happy.face(frame % 4 == 3));
    let kind = msg.map(|m| m.kind);
    let footers: &[&str] =
        if msg.is_some_and(|m| m.options.is_some()) { &i18n::FOOTER_ASK } else { &i18n::FOOTER_ASSISTANT };

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
        // live inside the bubble. Fixed 4 body rows — no reflow.
        let mut face_str = crate::species::render_tiny_face(pet.species, face.0, face.1);
        if let Some(k) = kind {
            face_str = animate_tiny(face_str, k, frame);
        }
        let bubble_w = (iw - face_str.chars().count() - 1).min(58);
        let inner = bubble_w.saturating_sub(4);
        let color = msg.map(|m| kind_color(m.kind)).unwrap_or(Color::DarkGrey);
        let mut body: Vec<Line> = Vec::new();
        match msg {
            Some(m) => {
                if m.options.is_some() && !m.from.is_empty() {
                    body.push(tinted(format!("{} {}:", m.from, i18n::ASKS_VERB), Color::DarkGrey));
                }
                body.extend(wrap(m.text, inner).into_iter().take(3 - body.len()).map(plain));
                while body.len() < 3 {
                    body.push(Vec::new());
                }
                let mut last: Line = Vec::new();
                if let Some(options) = m.options {
                    for (i, o) in options.iter().enumerate().take(9) {
                        last.push(chip(&(i + 1).to_string()));
                        last.push(seg(format!(" {o}   "), None));
                    }
                } else {
                    if !m.from.is_empty() {
                        last.push(seg(format!("{}: ", i18n::FROM_LABEL), Some(Color::DarkGrey)));
                        last.push(seg(m.from, Some(color)));
                        last.push(seg("   ", None));
                    }
                    last.push(seg(format!("{}: {}", i18n::TYPE_LABEL, m.kind_label), Some(Color::DarkGrey)));
                }
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
        // last resort: face + message + options as one left-aligned block
        let face_color = kind.map(kind_color).unwrap_or(Color::Green);
        let mut rows: Vec<Line> =
            vec![tinted(crate::species::render_tiny_face(pet.species, face.0, face.1), face_color)];
        if let Some(m) = msg {
            let width = iw.max(10).min(60);
            rows.extend(wrap(m.text, width).into_iter().take(3).map(plain));
            if let Some(options) = m.options {
                let mut opts: Line = Vec::new();
                for (i, o) in options.iter().enumerate().take(9) {
                    opts.push(chip(&(i + 1).to_string()));
                    opts.push(seg(format!(" {o} "), None));
                }
                rows.push(opts);
            }
        } else {
            rows.push(tinted(i18n::NO_MESSAGES, Color::DarkGrey));
        }
        rows.truncate(ih.saturating_sub(1).max(1));
        content = pad_block(rows);
    }
    draw_screen(out, &content, footers)
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
        HomeView { log, clock_text: "12:00", hour: 12, timer: None, progress: None }
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
    fn clock_falls_back_gracefully() {
        let (text, hour) = fetch_clock();
        assert!(text == "--:--" || text.len() == 5);
        assert!(hour < 24);
    }
}
