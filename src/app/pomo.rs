//! The pomodoro cycle: alternating focus and break phases that outlive the
//! process (Android kills Termux whenever it likes), so it round-trips through
//! `state::PomoState`.

use crate::state;

// Index-aligned with i18n::t().pomo_preset_labels: (focus, break) in seconds.
pub const POMO_PRESETS: [(u64, u64); 3] = [(25 * 60, 5 * 60), (50 * 60, 10 * 60), (15 * 60, 3 * 60)];

// A running pomodoro: alternates focus/break phases until stopped.
pub struct Pomo {
    pub work: u64,
    pub rest: u64,
    pub focus: bool,
    pub until: u64,
    pub cycles: u32, // 1-based: which focus block we are on
}

impl From<state::PomoState> for Pomo {
    fn from(p: state::PomoState) -> Self {
        Pomo { work: p.work, rest: p.rest, focus: p.focus, until: p.until, cycles: p.cycles }
    }
}

impl Pomo {
    pub(super) fn saved(&self) -> state::PomoState {
        state::PomoState {
            work: self.work,
            rest: self.rest,
            focus: self.focus,
            until: self.until,
            cycles: self.cycles,
        }
    }
}

// Flips the phase when the current one ended; returns the phase just entered
// (true = focus).
pub fn pomo_tick(p: &mut Pomo, now: u64) -> Option<bool> {
    if p.until > now {
        return None;
    }
    p.focus = !p.focus;
    p.until = now + if p.focus { p.work } else { p.rest };
    if p.focus {
        p.cycles += 1;
    }
    Some(p.focus)
}

pub fn mmss(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_has_a_label() {
        assert_eq!(POMO_PRESETS.len(), crate::i18n::t().pomo_preset_labels.len());
    }

    #[test]
    fn mmss_formats_minutes_and_hours() {
        assert_eq!(mmss(0), "00:00");
        assert_eq!(mmss(1062), "17:42");
        assert_eq!(mmss(3661), "1:01:01");
    }

    #[test]
    fn pomo_tick_alternates_phases_with_their_own_durations() {
        let mut p = Pomo { work: 1500, rest: 300, focus: true, until: 100, cycles: 1 };
        assert_eq!(pomo_tick(&mut p, 50), None); // still running
        assert_eq!(pomo_tick(&mut p, 100), Some(false)); // break starts
        assert_eq!((p.until, p.cycles), (400, 1));
        assert_eq!(pomo_tick(&mut p, 400), Some(true)); // focus resumes
        assert_eq!((p.until, p.cycles), (1900, 2));
    }
}
