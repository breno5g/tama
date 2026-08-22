mod app;
mod assistant;
mod http;
mod cli;
mod i18n;
mod pet;
mod species;
mod state;
mod ui;

use std::io;

use crossterm::{cursor, execute, terminal};

use pet::Mood;
use species::{render_art, render_tiny, ArtSize, SPECIES};

fn gallery() {
    for s in SPECIES {
        println!("\n== {} ==", i18n::species_name(s));
        for line in render_art(s, Mood::Happy, 0, ArtSize::Large) {
            println!("  {line}");
        }
        println!("\n  {}", i18n::t().gallery_small);
        for line in render_art(s, Mood::Happy, 0, ArtSize::Small) {
            println!("  {line}");
        }
        println!("\n  {} {}", i18n::t().gallery_mini, render_tiny(s, Mood::Happy, 0));
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = cli::run(&args) {
        std::process::exit(code);
    }
    if args.iter().any(|a| a == "--gallery") {
        gallery();
        return Ok(());
    }

    let (mut pet, is_new) = match state::load() {
        Some(pet) => (pet, false),
        None => (pet::Pet::default(), true),
    };
    let mut out = io::stdout();

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ui::restore_terminal();
        default_hook(info);
    }));

    terminal::enable_raw_mode()?;
    execute!(out, terminal::EnterAlternateScreen, cursor::Hide)?;

    let result = app::run(&mut out, &mut pet, is_new);

    ui::restore_terminal();
    state::save(&mut pet)?;
    result
}
