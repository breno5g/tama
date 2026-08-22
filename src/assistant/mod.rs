//! Assistant mode: external programs send one flat JSON object per line —
//! through the input pipe or an HTTP POST — and answers to questions go to
//! the output file, also as JSON lines. Invalid pipe lines are silently
//! ignored, per the design contract (HTTP gets a 400 instead).
//!
//! The schema is English (`message`, `from`, `actions`, ...); everything the
//! user READS stays in i18n.

mod answer;
mod json;
#[cfg(test)]
mod tests;

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::{data_dir, input_path};

pub use answer::{wait_answer, write_answer};
pub use json::{json_escape, json_fields};

use json::get;

// Protocol value, not UI text: what a discarded ask answers to its caller.
pub const ANSWER_IGNORED: &str = "ignored";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Info,
    Success,
    Warn,
    Error,
}

impl Kind {
    pub fn from_id(s: &str) -> Kind {
        match s {
            "success" => Kind::Success,
            "warn" => Kind::Warn,
            "error" => Kind::Error,
            _ => Kind::Info,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Say { text: String, from: String, kind: Kind },
    // `input`: a typed answer is accepted — shown as one more numbered option
    Ask { text: String, options: Vec<String>, id: String, from: String, expires: Option<u64>, input: bool },
    Action(String),
    Progress { from: String, pct: u8 },
    Reminder { text: String, at: u64 },
    Timer { until: u64 },
    Pomodoro { work: u64, rest: u64 },
    PomodoroOff,
}

// "30s", "10m", "1h" → seconds.
pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: u64 = num.parse().ok()?;
    match unit {
        "s" => Some(n),
        "m" => Some(n * 60),
        "h" => Some(n * 3600),
        _ => None,
    }
}

// One line may carry a pet `command` AND a message; both are delivered.
pub fn parse_msgs(line: &str, now: u64) -> Vec<Msg> {
    let Some(fields) = json_fields(line) else { return Vec::new() };
    let mut out = Vec::new();
    if let Some(c) = get(&fields, "command") {
        out.push(Msg::Action(c));
    }
    if let Some(m) = msg_from_fields(&fields, now) {
        out.push(m);
    }
    out
}

#[cfg(test)]
fn parse_line(line: &str, now: u64) -> Option<Msg> {
    msg_from_fields(&json_fields(line)?, now)
}

fn msg_from_fields(fields: &[(String, String)], now: u64) -> Option<Msg> {
    let from = get(fields, "from").unwrap_or_default();
    if let Some(text) = get(fields, "message") {
        // `actions` or `input` turns a message into a question; alone, speech.
        // Every ask also accepts a typed answer; empty options = text-only.
        let actions = get(fields, "actions");
        let input = get(fields, "input").as_deref() == Some("true");
        if actions.is_some() || input {
            let options: Vec<String> = actions
                .unwrap_or_default()
                .split('\n')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect();
            let id = get(fields, "id").unwrap_or_else(|| format!("ask-{now}"));
            let expires = get(fields, "expires").and_then(|e| e.parse().ok());
            return Some(Msg::Ask {
                text,
                options: if options.is_empty() && !input {
                    vec![crate::i18n::t().default_yes.into(), crate::i18n::t().default_no.into()]
                } else {
                    options
                },
                id,
                from,
                expires,
                input,
            });
        }
        return Some(Msg::Say { text, from, kind: Kind::from_id(&get(fields, "type").unwrap_or_default()) });
    }
    if let Some(p) = get(fields, "progress") {
        return Some(Msg::Progress { from, pct: p.parse::<u16>().ok()?.min(100) as u8 });
    }
    if let Some(text) = get(fields, "remind") {
        return Some(Msg::Reminder { text, at: now + parse_duration(&get(fields, "in")?)? });
    }
    if let Some(t) = get(fields, "timer") {
        return Some(Msg::Timer { until: now + parse_duration(&t)? });
    }
    if let Some(p) = get(fields, "pomodoro") {
        if p == "off" {
            return Some(Msg::PomodoroOff);
        }
        let rest = get(fields, "break").map_or(Some(300), |s| parse_duration(&s))?;
        return Some(Msg::Pomodoro { work: parse_duration(&p)?, rest });
    }
    None
}

// Ensures the input FIFO exists and streams its lines into the shared
// channel. The reader thread blocks on open/read (a FIFO with no writer
// blocks), so the main loop stays non-blocking via try_iter.
pub fn spawn_reader(tx: Sender<String>) {
    let path: PathBuf = input_path();
    let _ = std::fs::create_dir_all(data_dir());
    if !path.exists() {
        let _ = std::process::Command::new("mkfifo").arg(&path).status();
    }
    std::thread::spawn(move || loop {
        let Ok(f) = File::open(&path) else { return };
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
        // EOF: every writer closed; reopen and keep listening.
    });
}

pub fn now_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}
