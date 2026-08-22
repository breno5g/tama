//! Everything that arrives or comes due: external messages off the shared
//! channel, reminders, the timer, pomodoro phase flips, and the bookkeeping
//! that follows (expired asks, promoting the next message, saving the
//! schedule, the Android notification).

use std::io::{self, Write};
use std::time::Duration;

use crossterm::style::Color;

use super::inbox::pending_ask;
use super::pomo::{pomo_tick, Pomo};
use super::{notify_android, App, Msg, Screen, SAY_SECS};
use crate::assistant::{self, Kind};
use crate::i18n;
use crate::pet::{adj, FOODS};
use crate::state::{self, save};
use crate::ui;

// Updates the per-source progress list in place, keeping display order stable;
// true when the task hit 100% and was removed.
fn update_progress(progress: &mut Vec<(String, u8)>, from: &str, pct: u8) -> bool {
    if pct >= 100 {
        progress.retain(|(f, _)| f != from);
        return true;
    }
    match progress.iter_mut().find(|(f, _)| f == from) {
        Some(entry) => entry.1 = pct,
        None => progress.push((from.to_string(), pct)),
    }
    false
}

// Removes and returns the reminders that are due.
fn due_reminders(reminders: &mut Vec<(String, u64)>, now: u64) -> Vec<String> {
    let fired = reminders.iter().filter(|(_, at)| *at <= now).map(|(t, _)| t.clone()).collect();
    reminders.retain(|(_, at)| *at > now);
    fired
}

impl App<'_> {
    // Queues something for the pet to say and, if we are sitting on the home
    // screen, switches to assistant mode to show it. Every spontaneous message
    // (reminder, timer, phase flip, celebration) arrives this way.
    fn announce(&mut self, text: String, from: String, kind: Kind) {
        self.inbox.queue.push_back(Msg::Say { text, from, kind });
        if matches!(self.screen, Screen::Home) {
            self.screen = Screen::Assistant;
        }
    }

    pub(super) fn start_pomo(&mut self, work: u64, rest: u64) {
        self.pomo = Some(Pomo { work, rest, focus: true, until: self.now + work, cycles: 1 });
        self.schedule_dirty = true;
        self.pet.sleeping = false;
        self.log_at(i18n::t().msg_pomo_start.into(), None);
    }

    pub(super) fn stop_pomo(&mut self) {
        if self.pomo.take().is_some() {
            self.schedule_dirty = true;
            self.pet.sleeping = false;
            self.log_at(i18n::t().msg_pomo_stopped.into(), None);
        }
    }

    // Drains the shared channel (pipe + HTTP) and applies every message.
    pub(super) fn drain(&mut self, out: &mut impl Write) -> io::Result<()> {
        for line in self.rx.try_iter().collect::<Vec<_>>() {
            for msg in assistant::parse_msgs(&line, self.now) {
                self.handle_msg(out, msg)?;
            }
        }
        Ok(())
    }

    fn handle_msg(&mut self, out: &mut impl Write, msg: Msg) -> io::Result<()> {
        match msg {
            Msg::Say { ref text, ref from, kind } => {
                let sender = if from.is_empty() { i18n::t().unknown_sender } else { from };
                let line = vec![
                    ui::seg(format!("{} ", self.time), Some(Color::DarkGrey)),
                    ui::seg(format!("{sender}: "), Some(Color::DarkGrey)),
                    ui::seg(text.clone(), Some(ui::kind_color(kind))),
                ];
                self.log_line(line);
                self.inbox.queue.push_back(msg);
                if matches!(self.screen, Screen::Home) {
                    self.screen = Screen::Assistant;
                }
            }
            Msg::Ask { .. } => {
                self.inbox.queue.push_front(msg); // questions jump the queue
                let _ = out.write_all(b"\x07"); // flushed with the next frame

                // a question blocks its sender: interrupt from ANY screen
                if !matches!(self.screen, Screen::Assistant) {
                    self.prev_screen = self.screen;
                    self.screen = Screen::Assistant;
                }
            }
            Msg::Action(a) => match a.as_str() {
                "celebrate" => {
                    self.pet.happiness = adj(self.pet.happiness, 15);
                    self.pet.gain_xp(10);
                    let text = i18n::msg_celebrate(&self.pet.name);
                    self.announce(text, String::new(), Kind::Success);
                    save(self.pet)?;
                }
                "sleep" => {
                    self.pet.sleeping = true;
                    let text = i18n::msg_sleep(&self.pet.name, true);
                    self.log_at(text, None);
                }
                "wake" => {
                    self.pet.sleeping = false;
                    let text = i18n::msg_sleep(&self.pet.name, false);
                    self.log_at(text, None);
                }
                "feed" => {
                    self.pet.eat(&FOODS[0]);
                    let text = i18n::msg_action_fed(&self.pet.name);
                    let suffix = format!("(+15 {})", i18n::t().stat_labels[0]);
                    self.log_at(text, Some((suffix, Color::Green)));
                    save(self.pet)?;
                }
                _ => {}
            },
            Msg::Progress { from, pct } => {
                if update_progress(&mut self.progress, &from, pct) {
                    let text = i18n::msg_progress_done(&from);
                    self.log_at(text, Some(("(100%)".into(), Color::Green)));
                }
            }
            Msg::Reminder { text, at } => {
                self.reminders.push((text, at));
                self.schedule_dirty = true;
            }
            Msg::Timer { until } => {
                self.timer_until = Some(until);
                self.schedule_dirty = true;
            }
            Msg::Pomodoro { work, rest } => {
                self.start_pomo(work, rest);
                // a starting pomodoro opens its screen, like messages do
                if matches!(self.screen, Screen::Home) {
                    self.screen = Screen::Pomo(0);
                }
            }
            Msg::PomodoroOff => self.stop_pomo(),
        }
        Ok(())
    }

    // Reminders, the timer and the pomodoro phase clock.
    pub(super) fn fire_due(&mut self) -> io::Result<()> {
        let fired = due_reminders(&mut self.reminders, self.now);
        self.schedule_dirty |= !fired.is_empty();
        for text in fired {
            let text = i18n::msg_reminder(&text);
            self.announce(text, String::new(), Kind::Warn);
        }
        if self.timer_until.is_some_and(|u| u <= self.now) {
            self.timer_until = None;
            self.schedule_dirty = true;
            self.announce(i18n::t().msg_timer_done.into(), String::new(), Kind::Warn);
        }
        if let Some(p) = &mut self.pomo {
            if let Some(focus) = pomo_tick(p, self.now) {
                self.schedule_dirty = true;
                self.pet.sleeping = !focus; // the pet rests with you on breaks
                let text = if focus { i18n::t().msg_pomo_focus } else { i18n::t().msg_pomo_break };
                if matches!(self.screen, Screen::Pomo(_)) {
                    // the screen itself shows the change (title, color, pet);
                    // just keep the record in the log
                    self.log_at(text.into(), None);
                } else {
                    let kind = if focus { Kind::Success } else { Kind::Warn };
                    self.announce(text.into(), i18n::t().pomo_from.into(), kind);
                }
            }
        }
        // A pomodoro break IS nap time: overrides the full-energy auto-wake so
        // the pet keeps napping until the break actually ends.
        if self.pomo.as_ref().is_some_and(|p| !p.focus) {
            self.pet.sleeping = true;
        }
        Ok(())
    }

    // Expired asks vanish everywhere, even while another screen is up.
    pub(super) fn purge_expired(&mut self) {
        for from in self.inbox.purge_expired(self.now) {
            let sender = if from.is_empty() { i18n::t().unknown_sender.to_string() } else { from };
            let text = i18n::msg_ask_expired(&sender);
            self.log_at(text, None);
        }
    }

    // A spoken message expires on its own; questions wait for an answer.
    pub(super) fn promote_assistant(&mut self) {
        if !matches!(self.screen, Screen::Assistant) {
            return;
        }
        if let Some((Msg::Say { .. }, at)) = &self.inbox.current {
            if at.elapsed() >= Duration::from_secs(SAY_SECS) {
                self.inbox.current = None;
            }
        }
        self.inbox.promote();
        // cursor and typing buffer belong to ONE question: a new one on screen
        // starts clean, and a text-only ask opens the field itself
        match &self.inbox.current {
            Some((Msg::Ask { id, options, .. }, _)) if *id != self.ask_id => {
                self.ask_id = id.clone();
                self.ask_sel = 0;
                self.input = options.is_empty().then(String::new);
            }
            Some((Msg::Ask { .. }, _)) => {}
            _ => {
                self.ask_id.clear();
                self.input = None;
            }
        }
        if self.inbox.is_empty() {
            self.screen = self.prev_screen;
            self.prev_screen = Screen::Home;
        }
    }

    pub(super) fn flush_schedule(&mut self) {
        if !self.schedule_dirty {
            return;
        }
        let _ = state::save_schedule(&state::Schedule {
            reminders: self.reminders.clone(),
            timer: self.timer_until,
            pomo: self.pomo.as_ref().map(Pomo::saved),
        });
        self.schedule_dirty = false;
    }

    // One notification tracks "there is a question waiting", whatever ended it:
    // answered, ignored, expired or the app closing.
    pub(super) fn sync_notification(&mut self) {
        let pending = pending_ask(&self.inbox);
        if pending.is_some() != self.notified {
            notify_android(pending.as_deref());
            self.notified = pending.is_some();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_progress_tracks_sources_independently() {
        let mut p: Vec<(String, u8)> = Vec::new();
        assert!(!update_progress(&mut p, "a", 10));
        assert!(!update_progress(&mut p, "b", 20));
        assert!(!update_progress(&mut p, "a", 50));
        assert_eq!(p, vec![("a".to_string(), 50), ("b".to_string(), 20)]);
        assert!(update_progress(&mut p, "a", 100)); // done → removed
        assert_eq!(p, vec![("b".to_string(), 20)]);
    }

    #[test]
    fn due_reminders_fire_once_and_keep_the_rest() {
        let mut r = vec![("já".to_string(), 100), ("depois".to_string(), 500)];
        assert_eq!(due_reminders(&mut r, 200), vec!["já".to_string()]);
        assert_eq!(r.len(), 1);
        assert!(due_reminders(&mut r, 200).is_empty());
    }
}
