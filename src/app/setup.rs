//! First-run flows that own the whole screen: picking a species and naming the
//! pet. They run their own event loop before the main one starts.

use std::io::{self, Write};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::Color;

use super::screen::grid_step;
use crate::i18n;
use crate::pet::Mood;
use crate::species::{render_art, render_tiny, ArtSize, Species, SPECIES};
use crate::ui::{draw_screen, plain, seg, tinted, Line};

// Grid picker from the controls redesign: every species visible at once,
// with an animated preview of the highlighted one below.
pub fn pick_species(out: &mut impl Write, start: Species) -> io::Result<Species> {
    let mut idx = SPECIES.iter().position(|&s| s == start).unwrap_or(0);
    let mut frame = 0usize;
    const CELL: usize = 15;
    loop {
        let (tcols, trows) = crossterm::terminal::size()?;
        let (iw, ih) = (tcols.saturating_sub(2) as usize, trows.saturating_sub(2) as usize);
        let cols = (iw / CELL).clamp(1, 5).min(SPECIES.len());
        let species = SPECIES[idx];

        let mut content: Vec<Line> = vec![
            vec![
                seg(i18n::t().picker_title, Some(Color::Magenta)),
                seg(format!("  {} ({}/{})", i18n::species_name(species), idx + 1, SPECIES.len()), Some(Color::DarkGrey)),
            ],
            Vec::new(),
        ];
        for row_start in (0..SPECIES.len()).step_by(cols) {
            let mut faces: Line = Vec::new();
            let mut names: Line = Vec::new();
            for (offset, &sp) in SPECIES[row_start..(row_start + cols).min(SPECIES.len())].iter().enumerate() {
                let selected = row_start + offset == idx;
                faces.push(seg(
                    format!("{:^CELL$}", render_tiny(sp, Mood::Happy, if selected { frame } else { 0 })),
                    Some(if selected { Color::Cyan } else { Color::Green }),
                ));
                names.push(seg(
                    format!("{:^CELL$}", i18n::species_name(sp)),
                    Some(if selected { Color::Cyan } else { Color::DarkGrey }),
                ));
            }
            content.push(faces);
            content.push(names);
            content.push(Vec::new());
        }
        let preview = render_art(species, Mood::Happy, frame, ArtSize::Small);
        if ih >= content.len() + preview.len() + 1 && iw >= preview[0].chars().count() {
            content.extend(preview.iter().map(|l| plain(l.clone())));
        }
        draw_screen(out, &content, &i18n::t().footer_picker)?;

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Enter | KeyCode::Char(' ') => return Ok(SPECIES[idx]),
                        KeyCode::Esc => return Ok(start),
                        code => idx = grid_step(idx, SPECIES.len(), cols, code),
                    }
                }
            }
        } else {
            frame += 1;
        }
    }
}

pub fn ask_name(out: &mut impl Write, species: Species) -> io::Result<String> {
    let mut name = String::new();
    let mut frame = 0usize;
    loop {
        let mut content: Vec<Line> = vec![tinted(i18n::t().name_prompt, Color::Magenta), Vec::new()];
        content.extend(render_art(species, Mood::Happy, frame, ArtSize::Small).iter().map(|l| plain(l.clone())));
        content.push(Vec::new());
        content.push(tinted(format!("> {name}_"), Color::Cyan));
        draw_screen(out, &content, &i18n::t().footer_name)?;

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Enter => {
                            let name = name.trim().to_string();
                            return Ok(if name.is_empty() { i18n::t().default_name.to_string() } else { name });
                        }
                        KeyCode::Backspace => {
                            name.pop();
                        }
                        // restricted to keep the key=value state file unambiguous
                        KeyCode::Char(c)
                            if (c.is_alphanumeric() || c == ' ' || c == '-') && name.chars().count() < 12 =>
                        {
                            name.push(c);
                        }
                        _ => {}
                    }
                }
            }
        } else {
            frame += 1;
        }
    }
}
