//! Key handling. Returns `true` when the app should exit.
//!
//! The assistant screen's keys live in `keys_assistant.rs`; everything else is
//! here, one arm per screen.

use std::io::{self, Write};

use crossterm::event::{KeyCode, KeyEvent};

use super::pomo::POMO_PRESETS;
use super::screen::{actions_for, Action, Screen};
use super::App;
use crate::pet::FOODS;

impl App<'_> {
    pub(super) fn on_key(&mut self, out: &mut impl Write, k: KeyEvent) -> io::Result<bool> {
        match self.screen {
            Screen::Home => return self.on_key_home(out, k.code),
            Screen::Actions(sel) => self.on_key_actions(out, k.code, sel)?,
            Screen::Menu(sel) => self.on_key_menu(k.code, sel)?,
            Screen::Game => self.on_key_game(k.code)?,
            Screen::Pomo(sel) => self.on_key_pomo(k.code, sel),
            Screen::Assistant => return self.on_key_assistant(k),
        }
        Ok(false)
    }

    fn on_key_home(&mut self, out: &mut impl Write, code: KeyCode) -> io::Result<bool> {
        let zen = self.pet.zen;
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.quit();
                return Ok(true);
            }
            KeyCode::Char(' ') => self.screen = Screen::Actions(0),
            KeyCode::Char('a') => self.screen = Screen::Assistant,
            // legacy shortcuts, hidden from the footer but kept working
            KeyCode::Char('f') if !zen => self.screen = Screen::Menu(0),
            KeyCode::Char('m') if !zen => self.screen = Screen::Game,
            KeyCode::Char('p') if !zen => self.do_play()?,
            KeyCode::Char('s') if !zen => self.do_sleep(),
            KeyCode::Char('b') if !zen => self.do_bath()?,
            KeyCode::Char('z') => self.do_zen()?,
            KeyCode::Char('c') => self.do_switch(out)?,
            _ => {}
        }
        Ok(false)
    }

    fn on_key_actions(&mut self, out: &mut impl Write, code: KeyCode, sel: usize) -> io::Result<()> {
        let items = actions_for(self.pet.zen);
        let chosen = match code {
            KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                self.screen = Screen::Home;
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.screen = Screen::Actions((sel + items.len() - 1) % items.len());
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.screen = Screen::Actions((sel + 1) % items.len());
                None
            }
            KeyCode::Enter => items.get(sel).copied(),
            KeyCode::Char(c @ '1'..='9') => items.get(c as usize - '1' as usize).copied(),
            _ => None,
        };
        let Some(action) = chosen else { return Ok(()) };
        self.screen = Screen::Home;
        match action {
            Action::Feed => self.screen = Screen::Menu(0),
            Action::Game => self.screen = Screen::Game,
            Action::Assistant => self.screen = Screen::Assistant,
            Action::Pomo => self.screen = Screen::Pomo(0),
            Action::Play => self.do_play()?,
            Action::Sleep => self.do_sleep(),
            Action::Bath => self.do_bath()?,
            Action::Zen => self.do_zen()?,
            Action::Switch => self.do_switch(out)?,
        }
        Ok(())
    }

    fn on_key_menu(&mut self, code: KeyCode, sel: usize) -> io::Result<()> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Home,
            KeyCode::Up | KeyCode::Char('k') => {
                self.screen = Screen::Menu((sel + FOODS.len() - 1) % FOODS.len())
            }
            KeyCode::Down | KeyCode::Char('j') => self.screen = Screen::Menu((sel + 1) % FOODS.len()),
            KeyCode::Enter => {
                self.do_feed(sel)?;
                self.screen = Screen::Home;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_key_game(&mut self, code: KeyCode) -> io::Result<()> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => self.screen = Screen::Home,
            KeyCode::Char(c @ '1'..='3') => {
                self.do_game(c as usize - '1' as usize)?;
                self.screen = Screen::Home;
            }
            _ => {}
        }
        Ok(())
    }

    fn on_key_pomo(&mut self, code: KeyCode, sel: usize) {
        let idle = self.pomo.is_none();
        let start = match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.screen = Screen::Home;
                None
            }
            KeyCode::Up | KeyCode::Char('k') if idle => {
                self.screen = Screen::Pomo((sel + POMO_PRESETS.len() - 1) % POMO_PRESETS.len());
                None
            }
            KeyCode::Down | KeyCode::Char('j') if idle => {
                self.screen = Screen::Pomo((sel + 1) % POMO_PRESETS.len());
                None
            }
            // enter starts the selected preset — or stops the running cycle
            KeyCode::Enter => {
                if idle {
                    POMO_PRESETS.get(sel).copied()
                } else {
                    self.stop_pomo();
                    None
                }
            }
            KeyCode::Char(c @ '1'..='9') if idle => POMO_PRESETS.get(c as usize - '1' as usize).copied(),
            _ => None,
        };
        if let Some((work, rest)) = start {
            self.start_pomo(work, rest);
            // stay here: this screen IS the focus mode
        }
    }
}
