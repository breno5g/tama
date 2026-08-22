//! CLI subcommands for external integration: they only write JSON lines to
//! the input pipe (`tama ask` additionally waits for its answer line in the
//! output file and prints it to stdout). Flags and wire keys are English;
//! every printed string stays in i18n.

mod commands;

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::i18n;
use crate::state::input_path;

const PIPE_OPEN_TIMEOUT: Duration = Duration::from_secs(2);

// Returns Some(exit code) when args named a subcommand; None → run the TUI.
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first()?.as_str();
    let rest = &args[1..];
    match cmd {
        "say" => Some(commands::say(rest)),
        "ask" => Some(commands::ask(rest)),
        "remind" => Some(commands::remind(rest)),
        "timer" => Some(commands::timer(rest)),
        "do" => Some(commands::action(rest)),
        "watch" => Some(commands::watch(rest)),
        "pomodoro" => Some(commands::pomodoro(rest)),
        _ => None,
    }
}

// Flags that carry no value — skipping two args past them would eat the text.
pub(super) const BOOL_FLAGS: [&str; 1] = ["--input"];

pub(super) fn flag(rest: &[String], name: &str) -> Option<String> {
    rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).cloned()
}

// Every occurrence of a repeatable flag, in order.
pub(super) fn flags(rest: &[String], name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        if rest[i] == name {
            if let Some(v) = rest.get(i + 1) {
                out.push(v.clone());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

pub(super) fn positional(rest: &[String]) -> Option<String> {
    let mut i = 0;
    while i < rest.len() {
        if BOOL_FLAGS.contains(&rest[i].as_str()) {
            i += 1;
        } else if rest[i].starts_with("--") {
            i += 2;
        } else {
            return Some(rest[i].clone());
        }
    }
    None
}

pub(super) fn usage(msg: &str) -> i32 {
    eprintln!("{msg}");
    2
}

// A FIFO write blocks forever when no reader (the app) is attached, so the
// write runs in a thread raced against a timeout.
pub(super) fn send(line: String) -> i32 {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = OpenOptions::new()
            .write(true)
            .open(input_path())
            .and_then(|mut f| f.write_all(line.as_bytes()));
        let _ = tx.send(result);
    });
    match rx.recv_timeout(PIPE_OPEN_TIMEOUT) {
        Ok(Ok(())) => 0,
        Ok(Err(e)) => usage(&format!("{}: {e}", i18n::t().cli_pipe_error)),
        Err(_) => usage(i18n::t().cli_not_running),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_and_positionals_parse_in_any_order() {
        let rest = v(&["--from", "ci", "build ok", "--type", "success"]);
        assert_eq!(positional(&rest), Some("build ok".to_string()));
        assert_eq!(flag(&rest, "--from"), Some("ci".to_string()));
        assert_eq!(flag(&rest, "--type"), Some("success".to_string()));
        assert_eq!(flag(&rest, "--in"), None);
    }

    #[test]
    fn valueless_flags_do_not_swallow_the_text() {
        let rest = v(&["--input", "escreva aí", "--from", "llm"]);
        assert_eq!(positional(&rest), Some("escreva aí".to_string()));
        assert_eq!(flag(&rest, "--from"), Some("llm".to_string()));
    }

    #[test]
    fn flags_collects_every_occurrence() {
        let rest = v(&["ok?", "--options", "permitir", "--from", "claude", "--options", "negar, talvez"]);
        assert_eq!(flags(&rest, "--options"), v(&["permitir", "negar, talvez"]));
        assert_eq!(flags(&rest, "--in"), Vec::<String>::new());
    }

    #[test]
    fn unknown_command_falls_through_to_tui() {
        assert_eq!(run(&v(&["--gallery"])), None);
        assert_eq!(run(&[]), None);
    }
}
