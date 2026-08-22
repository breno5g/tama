//! The message inbox: one message on screen at a time, the rest queued.
//! Questions jump the queue, because a question blocks the program that asked.
//!
//! Also the arrival notification. A question may land while you are in another
//! window — or another room, with the tablet on the desk — so the bell rings
//! and, on Termux, one Android notification tracks "something is waiting".

use std::collections::VecDeque;
use std::time::Instant;

use crate::assistant::{self, Msg};
use crate::ui;

// Everything the assistant flow needs in the main loop.
pub struct Inbox {
    pub queue: VecDeque<Msg>,
    pub current: Option<(Msg, Instant)>,
}

impl Inbox {
    pub fn new() -> Self {
        Inbox { queue: VecDeque::new(), current: None }
    }

    pub fn promote(&mut self) {
        if self.current.is_none() {
            self.current = self.queue.pop_front().map(|m| (m, Instant::now()));
        }
    }

    // Drops the current message; answers a discarded Ask so callers never hang.
    pub fn advance(&mut self) {
        if let Some((Msg::Ask { id, .. }, _)) = self.current.take() {
            let _ = assistant::write_answer(&id, assistant::ANSWER_IGNORED);
        }
    }

    pub fn clear(&mut self) {
        self.advance();
        for m in self.queue.drain(..) {
            if let Msg::Ask { id, .. } = m {
                let _ = assistant::write_answer(&id, assistant::ANSWER_IGNORED);
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.current.is_none() && self.queue.is_empty()
    }

    // Drops expired asks WITHOUT writing an answer — the CLI side already gave
    // up and printed its default; returns the senders for the log.
    pub fn purge_expired(&mut self, now: u64) -> Vec<String> {
        let expired = |m: &Msg| matches!(m, Msg::Ask { expires: Some(e), .. } if *e <= now);
        let mut froms = Vec::new();
        if self.current.as_ref().is_some_and(|(m, _)| expired(m)) {
            if let Some((Msg::Ask { from, .. }, _)) = self.current.take() {
                froms.push(from);
            }
        }
        self.queue.retain(|m| {
            if expired(m) {
                if let Msg::Ask { from, .. } = m {
                    froms.push(from.clone());
                }
                return false;
            }
            true
        });
        froms
    }
}

pub fn queue_preview(m: &Msg) -> Option<String> {
    match m {
        Msg::Say { text, from, .. } | Msg::Ask { text, from, .. } => {
            Some(if from.is_empty() { text.clone() } else { format!("{from}: {text}") })
        }
        _ => None,
    }
}

pub fn pending_ask(inbox: &Inbox) -> Option<String> {
    let ask = |m: &Msg| match m {
        Msg::Ask { text, from, .. } => {
            Some(if from.is_empty() { text.clone() } else { format!("{from}: {text}") })
        }
        _ => None,
    };
    inbox
        .current
        .as_ref()
        .and_then(|(m, _)| ask(m))
        .or_else(|| inbox.queue.iter().find_map(ask))
}

// The choice list of the question on screen: the sender's options plus the
// "escrever" entry when it accepts free text.
pub fn ask_options(inbox: &Inbox) -> Option<Vec<String>> {
    match &inbox.current {
        Some((Msg::Ask { options, input, .. }, _)) => Some(ui::option_labels(options, *input)),
        _ => None,
    }
}

// A question blocks its sender and you may be in another window — or another
// room, with the tablet on the desk. The bell rings on arrival; on Termux the
// pending question also becomes an Android notification (one, replaced in
// place) that goes away once nothing is pending.
fn notify_cmd(pending: Option<&str>) -> std::process::Command {
    match pending {
        Some(text) => {
            let mut c = std::process::Command::new("termux-notification");
            c.args(["--id", "tama-ask", "--title", "tama", "--content", text]);
            c
        }
        None => {
            let mut c = std::process::Command::new("termux-notification-remove");
            c.arg("tama-ask");
            c
        }
    }
}

pub fn notify_android(pending: Option<&str>) {
    let mut cmd = notify_cmd(pending);
    // detached: the notification command must not stall the render loop, and
    // waiting in the thread reaps it instead of leaving a zombie behind
    std::thread::spawn(move || {
        let _ = cmd.status();
    });
}

// Quitting with a question on screen must not leave the notification behind;
// here the wait is fine (and necessary — the process is about to end).
pub fn notify_clear(notified: bool) {
    if notified {
        let _ = notify_cmd(None).status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::Kind;

    #[test]
    fn questions_jump_the_queue_and_says_expire() {
        let mut inbox = Inbox::new();
        inbox.queue.push_back(Msg::Say { text: "s".into(), from: String::new(), kind: Kind::Info });
        inbox.queue.push_front(Msg::Ask {
            text: "q".into(),
            options: vec!["sim".into()],
            id: "i".into(),
            from: String::new(),
            expires: None,
            input: false,
        });
        inbox.promote();
        assert!(matches!(inbox.current, Some((Msg::Ask { .. }, _))));
    }

    #[test]
    fn purge_expired_drops_only_expired_asks_and_reports_senders() {
        let ask = |id: &str, from: &str, expires: Option<u64>| Msg::Ask {
            text: "q".into(),
            options: vec!["sim".into()],
            id: id.into(),
            from: from.into(),
            expires,
            input: false,
        };
        let mut inbox = Inbox::new();
        inbox.queue.push_back(ask("a", "claude", Some(100)));
        inbox.queue.push_back(ask("b", "ci", None));
        inbox.queue.push_back(ask("c", "outro", Some(500)));
        inbox.promote(); // "a" becomes current
        assert_eq!(inbox.purge_expired(100), vec!["claude".to_string()]);
        assert!(inbox.current.is_none());
        assert_eq!(inbox.queue.len(), 2); // "b" (sem expira) e "c" (ainda viva) ficam
        assert!(inbox.purge_expired(100).is_empty());
    }

    #[test]
    fn pending_ask_drives_the_notification_and_ignores_plain_messages() {
        let mut inbox = Inbox::new();
        assert_eq!(pending_ask(&inbox), None);
        inbox.queue.push_back(Msg::Say { text: "oi".into(), from: "ci".into(), kind: Kind::Info });
        assert_eq!(pending_ask(&inbox), None, "uma fala não é pergunta pendente");
        inbox.queue.push_back(Msg::Ask {
            text: "posso?".into(),
            options: vec!["sim".into()],
            id: "i".into(),
            from: "claude".into(),
            expires: None,
            input: false,
        });
        // queued or current, a waiting question always shows up
        assert_eq!(pending_ask(&inbox), Some("claude: posso?".to_string()));
        inbox.promote();
        inbox.current = None; // the say was promoted and dismissed
        inbox.promote();
        assert_eq!(pending_ask(&inbox), Some("claude: posso?".to_string()));
        inbox.advance();
        assert_eq!(pending_ask(&inbox), None);
    }

    #[test]
    fn queue_preview_only_covers_visible_messages() {
        assert!(queue_preview(&Msg::Say { text: "x".into(), from: "ci".into(), kind: Kind::Info })
            .is_some_and(|p| p == "ci: x"));
        assert!(queue_preview(&Msg::Action("dormir".into())).is_none());
    }
}
