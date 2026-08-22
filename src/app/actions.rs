//! What the action menu does to the pet, plus the rock-paper-scissors rules.

use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::style::Color;

use super::inbox::ask_options;
use super::{setup, App, Msg};
use crate::assistant;
use crate::i18n;
use crate::state::save;
use crate::ui::tinted;

#[derive(Debug, PartialEq)]
pub enum GameOutcome {
    Draw,
    Win,
    Loss,
}

// Rock-paper-scissors: 0 rock, 1 paper, 2 scissors; each pick beats the previous.
pub fn jokenpo(player: usize, pet_pick: usize) -> GameOutcome {
    if player == pet_pick {
        GameOutcome::Draw
    } else if player == (pet_pick + 1) % 3 {
        GameOutcome::Win
    } else {
        GameOutcome::Loss
    }
}

pub(super) fn random_pick() -> usize {
    // ponytail: subsecond nanos as rng — one dice roll doesn't justify a rand dependency
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as usize % 3
}

impl App<'_> {
    pub(super) fn do_play(&mut self) -> io::Result<()> {
        let leveled = self.pet.play();
        let text = i18n::msg_played(&self.pet.name);
        self.log_at(text, Some(("(+10 xp)".into(), Color::Cyan)));
        self.log_level_up(leveled);
        save(self.pet)
    }

    pub(super) fn do_bath(&mut self) -> io::Result<()> {
        let leveled = self.pet.bathe();
        let text = i18n::msg_bathed(&self.pet.name);
        self.log_at(text, Some((i18n::t().bath_suffix.into(), Color::Green)));
        self.log_level_up(leveled);
        save(self.pet)
    }

    pub(super) fn do_sleep(&mut self) {
        self.pet.sleeping = !self.pet.sleeping;
        let text = i18n::msg_sleep(&self.pet.name, self.pet.sleeping);
        self.log_at(text, None);
    }

    pub(super) fn do_zen(&mut self) -> io::Result<()> {
        self.pet.zen = !self.pet.zen;
        self.pet.sleeping = false;
        let text = i18n::msg_zen(self.pet.zen);
        self.log_at(text, None);
        save(self.pet)
    }

    pub(super) fn do_switch(&mut self, out: &mut impl Write) -> io::Result<()> {
        let new = setup::pick_species(out, self.pet.species)?;
        if new != self.pet.species {
            self.pet.species = new;
            let text = i18n::msg_became(&self.pet.name, new);
            self.log_at(text, None);
            save(self.pet)?;
        }
        Ok(())
    }

    pub(super) fn do_feed(&mut self, sel: usize) -> io::Result<()> {
        let food = &crate::pet::FOODS[sel];
        let leveled = self.pet.eat(food);
        let labels = i18n::t().stat_labels;
        let suffix = if food.hunger > 0 {
            format!("(+{} {})", food.hunger, labels[0])
        } else {
            format!("(+{} {})", food.energy, labels[2])
        };
        let text = i18n::msg_fed(i18n::t().food_names[sel], &self.pet.name);
        self.log_at(text, Some((suffix, Color::Green)));
        self.log_level_up(leveled);
        save(self.pet)
    }

    // The pet WINNING makes the pet happier — by design.
    pub(super) fn do_game(&mut self, player: usize) -> io::Result<()> {
        let pet_pick = random_pick();
        let s = i18n::t();
        let (label, happy, xp) = match jokenpo(player, pet_pick) {
            GameOutcome::Draw => (s.game_draw, 5, 5),
            GameOutcome::Win => (s.game_win, 5, 10),
            GameOutcome::Loss => (s.game_loss, 15, 20),
        };
        self.pet.happiness = crate::pet::adj(self.pet.happiness, happy);
        self.pet.energy = crate::pet::adj(self.pet.energy, -5);
        let leveled = self.pet.gain_xp(xp);
        let text = i18n::msg_game(s.hands[player], s.hands[pet_pick], label);
        self.log_at(text, Some((format!("(+{xp} xp)"), Color::Cyan)));
        self.log_level_up(leveled);
        save(self.pet)
    }

    fn log_level_up(&mut self, leveled: bool) {
        if leveled {
            let text = i18n::msg_level_up(&self.pet.name, self.pet.level);
            self.log_at(text, None);
        }
    }

    // Answers the current question — by picked option or typed text, same path.
    pub(super) fn answer_ask(&mut self, answer: &str) {
        let Some((Msg::Ask { text, id, .. }, _)) = &self.inbox.current else { return };
        let entry = i18n::msg_answered(text, answer);
        match assistant::write_answer(id, answer) {
            Ok(()) => self.log_at(entry, None),
            Err(e) => {
                let line = tinted(format!("{entry} ({e})"), Color::Red);
                self.log_line(line);
            }
        }
        self.inbox.current = None;
    }

    // The option the cursor is on, or the one a number key picked.
    pub(super) fn pick_option(&mut self, idx: usize) {
        let picked = ask_options(&self.inbox).and_then(|o| o.get(idx).cloned());
        match picked {
            Some(o) if o == i18n::t().option_write => self.input = Some(String::new()),
            Some(option) => self.answer_ask(&option),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jokenpo_covers_all_nine_combinations() {
        let mut draws = 0;
        let mut wins = 0;
        let mut losses = 0;
        for p in 0..3 {
            for c in 0..3 {
                match jokenpo(p, c) {
                    GameOutcome::Draw => draws += 1,
                    GameOutcome::Win => wins += 1,
                    GameOutcome::Loss => losses += 1,
                }
            }
        }
        assert_eq!((draws, wins, losses), (3, 3, 3));
    }

    #[test]
    fn jokenpo_paper_beats_rock() {
        assert_eq!(jokenpo(1, 0), GameOutcome::Win);
        assert_eq!(jokenpo(0, 1), GameOutcome::Loss);
        assert_eq!(jokenpo(2, 2), GameOutcome::Draw);
    }

    #[test]
    fn random_pick_is_a_valid_hand() {
        for _ in 0..50 {
            assert!(random_pick() < 3);
        }
    }
}
