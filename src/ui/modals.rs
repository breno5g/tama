//! Modal screens drawn over a dimmed home: the actions list, the food menu and
//! the rock-paper-scissors prompt.

use std::io::{self, Write};

use crossterm::style::Color;
use crossterm::terminal;

use super::home::build_home;
use super::line::{chip, dim, plain, seg};
use super::panel::{boxed, overlay};
use super::screen::draw_screen;
use super::{HomeView, Line};
use crate::i18n;
use crate::pet::{Pet, FOODS};

fn food_effects(food: &crate::pet::Food) -> Line {
    let mut l: Line = Vec::new();
    for (delta, label) in [
        (food.hunger, i18n::t().stat_labels[0]),
        (food.happiness, i18n::t().stat_labels[1]),
        (food.energy, i18n::t().stat_labels[2]),
        (food.hygiene, i18n::t().stat_labels[3]),
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

// Index-aligned with app::Action and i18n::t().action_labels.
pub const ACTION_GLYPHS: [&str; 9] = [r"\∴/", "(o)", "z Z", "oOo", "1v1", "(!)", "(*)", "-_-", "<=>"];

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
            seg(i18n::t().action_labels[action], if selected { Some(Color::Cyan) } else { None }),
        ]);
    }
    let modal = boxed(Some((i18n::t().actions_title, Color::Magenta)), Color::Magenta, &body, w);
    let backdrop = dim(&build_home(pet, frame, view, iw, ih));
    let content = overlay(backdrop, &modal);
    draw_screen(out, &content, &i18n::t().footer_actions)
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
            seg(format!("{:<17}", i18n::t().food_names[i]), if selected { Some(Color::Cyan) } else { None }),
        ];
        l.extend(food_effects(food));
        body.push(l);
    }
    let modal = boxed(Some((i18n::t().menu_title, Color::Magenta)), Color::Magenta, &body, w);
    let content = overlay(dim(&build_home(pet, frame, view, iw, ih)), &modal);
    draw_screen(out, &content, &i18n::t().footer_menu)
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
            seg(format!(" {}   ", i18n::t().hands[0]), None),
            chip("2"),
            seg(format!(" {}   ", i18n::t().hands[1]), None),
            chip("3"),
            seg(format!(" {}", i18n::t().hands[2]), None),
        ],
    ];
    let modal = boxed(Some((i18n::t().game_title, Color::Magenta)), Color::Magenta, &body, w);
    let content = overlay(dim(&build_home(pet, frame, view, iw, ih)), &modal);
    draw_screen(out, &content, &i18n::t().footer_game)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ACTION_GLYPHS, i18n's action_labels and app::ACTIONS_ALL are index-aligned.
    // Each side asserts against the i18n table, which keeps all three pinned.
    #[test]
    fn a_glyph_and_a_label_for_every_action() {
        assert_eq!(ACTION_GLYPHS.len(), i18n::t().action_labels.len());
        assert_eq!(FOOD_ICONS.len(), i18n::t().food_names.len());
        assert_eq!(FOOD_ICONS.len(), FOODS.len());
    }
}
