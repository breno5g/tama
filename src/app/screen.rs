//! Screens and the action menu. `Screen` is the mode the app is in (some carry
//! their own cursor); `Action` is what the menu can trigger.

use crossterm::event::KeyCode;

#[derive(Clone, Copy)]
pub enum Screen {
    Home,
    Actions(usize),
    Menu(usize),
    Game,
    Assistant,
    Pomo(usize),
}

// Index-aligned with i18n::t().action_labels and ui::ACTION_GLYPHS.
#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Feed,
    Play,
    Sleep,
    Bath,
    Game,
    Assistant,
    Pomo,
    Zen,
    Switch,
}

pub const ACTIONS_ALL: [Action; 9] = [
    Action::Feed,
    Action::Play,
    Action::Sleep,
    Action::Bath,
    Action::Game,
    Action::Assistant,
    Action::Pomo,
    Action::Zen,
    Action::Switch,
];

pub const ACTIONS_ZEN: [Action; 4] = [Action::Assistant, Action::Pomo, Action::Zen, Action::Switch];

pub fn actions_for(zen: bool) -> &'static [Action] {
    if zen { &ACTIONS_ZEN } else { &ACTIONS_ALL }
}

// Grid navigation for the species picker: ←→ wrap linearly, ↑↓ move by row.
pub fn grid_step(idx: usize, len: usize, cols: usize, code: KeyCode) -> usize {
    match code {
        KeyCode::Left | KeyCode::Char('h') => (idx + len - 1) % len,
        KeyCode::Right | KeyCode::Char('l') => (idx + 1) % len,
        KeyCode::Up | KeyCode::Char('k') if idx >= cols => idx - cols,
        KeyCode::Down | KeyCode::Char('j') if idx + cols < len => idx + cols,
        _ => idx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_step_navigates_two_axes_with_wrap() {
        use KeyCode::*;
        let (len, cols) = (10, 5);
        assert_eq!(grid_step(0, len, cols, Right), 1);
        assert_eq!(grid_step(0, len, cols, Left), 9);
        assert_eq!(grid_step(9, len, cols, Right), 0);
        assert_eq!(grid_step(2, len, cols, Down), 7);
        assert_eq!(grid_step(7, len, cols, Up), 2);
        assert_eq!(grid_step(2, len, cols, Up), 2); // top row stays
        assert_eq!(grid_step(7, len, cols, Down), 7); // bottom row stays
    }

    #[test]
    fn action_tables_stay_index_aligned() {
        assert_eq!(ACTIONS_ALL.len(), crate::i18n::t().action_labels.len());
        for (i, a) in ACTIONS_ALL.iter().enumerate() {
            assert_eq!(*a as usize, i);
        }
        for a in ACTIONS_ZEN {
            assert!(ACTIONS_ALL.contains(&a));
        }
    }
}
