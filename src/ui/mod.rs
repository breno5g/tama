//! Rendering. Everything on screen is a `Vec<Line>` built by a `build_*`
//! function and handed to `draw_screen`, which frames and centers it. Those
//! build functions are pure, which is what makes the layout testable without
//! a terminal.
//!
//! Layout rule enforced throughout: dynamic content NEVER resizes the layout.
//! Every variable-length section — speech bubble, option list, event log,
//! progress bars, the sleeping "z Z z" — has reserved space, so nothing jumps.

mod answer;
mod assistant;
mod bigtime;
mod bubble;
mod clock;
mod expression;
mod header;
mod home;
mod line;
mod modals;
mod panel;
mod pomo;
mod screen;
mod stats;
#[cfg(test)]
mod testutil;

use std::collections::VecDeque;

use crossterm::style::Color;

pub type Seg = (String, Option<Color>, Option<Color>); // text, fg, bg
pub type Line = Vec<Seg>;

pub use answer::option_labels;
pub use assistant::{draw_assistant, AssistantMsg};
pub use clock::Clock;
pub use expression::kind_color;
pub use home::draw_home;
pub use line::{plain, seg, tinted};
pub use modals::{draw_actions, draw_game, draw_menu};
pub use pomo::{draw_pomo, PomoRun};
pub use screen::{draw_screen, restore_terminal};
pub use stats::{mood_color, progress_line};

// Everything draw_home needs beyond the pet itself.
pub struct HomeView<'a> {
    pub log: &'a VecDeque<Line>,
    pub clock_text: &'a str,
    pub hour: u8,
    pub timer: Option<(&'static str, String)>, // label ("timer"/"foco"/"pausa"), countdown
    pub progress: Vec<Line>,
}
