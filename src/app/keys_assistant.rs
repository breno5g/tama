//! Keys on the assistant screen: the option cursor, the numbered shortcuts, and
//! the free-text field.
//!
//! While the field is open EVERY key belongs to it — that branch is checked
//! before the shortcut table, or typing "q" would quit the app mid-sentence.

use std::io;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::inbox::ask_options;
use super::screen::Screen;
use super::{App, Msg, ANSWER_CAP};

impl App<'_> {
    pub(super) fn on_key_assistant(&mut self, k: KeyEvent) -> io::Result<bool> {
        if self.input.is_some() {
            self.on_key_input(k);
            return Ok(false);
        }
        match k.code {
            KeyCode::Char('q') => {
                self.quit();
                return Ok(true);
            }
            KeyCode::Char('a') => {
                self.screen = self.prev_screen;
                self.prev_screen = Screen::Home;
            }
            KeyCode::Char('x') => {
                self.inbox.clear();
                self.screen = self.prev_screen;
                self.prev_screen = Screen::Home;
            }
            // shortcut for the same "write" entry listed in the options
            KeyCode::Char('t') if matches!(self.inbox.current, Some((Msg::Ask { input: true, .. }, _))) => {
                self.input = Some(String::new());
            }
            // cursor over the option list, like the actions menu; the list
            // scrolls when it is longer than the visible slots
            KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                if let Some(len) = ask_options(&self.inbox).map(|o| o.len()) {
                    let back = matches!(k.code, KeyCode::Up | KeyCode::Char('k'));
                    self.ask_sel =
                        if back { (self.ask_sel + len - 1) % len } else { (self.ask_sel + 1) % len };
                }
            }
            KeyCode::Enter => match ask_options(&self.inbox) {
                Some(_) => self.pick_option(self.ask_sel),
                // no question on screen: enter dismisses a message
                None => {
                    if matches!(self.inbox.current, Some((Msg::Say { .. }, _))) {
                        self.inbox.current = None;
                    }
                }
            },
            KeyCode::Esc => self.inbox.advance(),
            // The numbered list is options + (when free text is accepted) one
            // last "write" entry, which opens the field instead of answering.
            KeyCode::Char(c @ '1'..='9') => self.pick_option(c as usize - '1' as usize),
            _ => {}
        }
        Ok(false)
    }

    fn on_key_input(&mut self, k: KeyEvent) {
        let buf = self.input.as_mut().expect("only called with the field open");
        match k.code {
            // alt+enter breaks the line; plain enter sends, which is the
            // reachable one on a phone keyboard
            KeyCode::Enter if k.modifiers.contains(KeyModifiers::ALT) && buf.chars().count() < ANSWER_CAP => {
                buf.push('\n');
            }
            KeyCode::Enter if !buf.trim().is_empty() => {
                let answer = buf.trim().to_string();
                self.answer_ask(&answer);
                self.input = None;
            }
            KeyCode::Backspace => {
                buf.pop();
            }
            // esc leaves the field; on a text-only ask there is nothing to go
            // back to, so it discards the question
            KeyCode::Esc => {
                let text_only =
                    matches!(&self.inbox.current, Some((Msg::Ask { options, .. }, _)) if options.is_empty());
                self.input = None;
                if text_only {
                    self.inbox.advance();
                }
            }
            KeyCode::Char(c) if buf.chars().count() < ANSWER_CAP => buf.push(c),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::i18n;

    #[test]
    fn the_write_entry_is_matched_by_value_not_by_index() {
        // pick_option compares the chosen label against this string, so the
        // locale files must never leave it blank or an empty option would open
        // the typing field.
        assert!(!i18n::t().option_write.is_empty());
    }
}
