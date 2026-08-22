//! CLI subcommands for external integration: they only write JSON lines to
//! the input pipe (`tama ask` additionally waits for its answer line in the
//! output file and prints it to stdout). Flags and wire keys are English;
//! every printed string stays in i18n.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::assistant::{json_escape, now_epoch, wait_answer};
use crate::i18n;
use crate::state::{input_path, output_path};

const PIPE_OPEN_TIMEOUT: Duration = Duration::from_secs(2);

// Returns Some(exit code) when args named a subcommand; None → run the TUI.
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first()?.as_str();
    let rest = &args[1..];
    match cmd {
        "say" => Some(say(rest)),
        "ask" => Some(ask(rest)),
        "remind" => Some(remind(rest)),
        "timer" => Some(timer(rest)),
        "do" => Some(action(rest)),
        "watch" => Some(watch(rest)),
        "pomodoro" => Some(pomodoro(rest)),
        _ => None,
    }
}

fn flag(rest: &[String], name: &str) -> Option<String> {
    rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).cloned()
}

// Every occurrence of a repeatable flag, in order.
fn flags(rest: &[String], name: &str) -> Vec<String> {
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

fn positional(rest: &[String]) -> Option<String> {
    let mut i = 0;
    while i < rest.len() {
        if rest[i].starts_with("--") {
            i += 2;
        } else {
            return Some(rest[i].clone());
        }
    }
    None
}

fn usage(msg: &str) -> i32 {
    eprintln!("{msg}");
    2
}

// A FIFO write blocks forever when no reader (the app) is attached, so the
// write runs in a thread raced against a timeout.
fn send(line: String) -> i32 {
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
        Ok(Err(e)) => usage(&format!("{}: {e}", i18n::CLI_PIPE_ERROR)),
        Err(_) => usage(i18n::CLI_NOT_RUNNING),
    }
}

fn say(rest: &[String]) -> i32 {
    let Some(text) = positional(rest) else { return usage(i18n::CLI_USAGE_SAY) };
    let from = flag(rest, "--from").unwrap_or_default();
    let kind = flag(rest, "--type").unwrap_or_else(|| "info".to_string());
    send(say_line(&text, &kind, &from))
}

// Options travel as one JSON string with a literal \n between them: a single
// --options keeps the comma-split shorthand, repeated flags are literal (so
// an option may contain commas).
fn ask_options(opts: &[String]) -> String {
    match opts {
        [] => "sim\nnão".into(),
        [one] => one.split(',').map(str::trim).collect::<Vec<_>>().join("\n"),
        many => many.join("\n"),
    }
}

fn ask(rest: &[String]) -> i32 {
    let Some(text) = positional(rest) else { return usage(i18n::CLI_USAGE_ASK) };
    let options = ask_options(&flags(rest, "--options"));
    let from = flag(rest, "--from").unwrap_or_default();
    let id = flag(rest, "--id").unwrap_or_else(|| {
        format!("ask-{}-{}", now_epoch(), std::process::id())
    });
    let deadline = match flag(rest, "--timeout") {
        Some(t) => match crate::assistant::parse_duration(&t) {
            Some(secs) => Some(now_epoch() + secs),
            None => return usage(i18n::CLI_USAGE_ASK),
        },
        None => None,
    };
    let expires = deadline.map(|d| format!(",\"expires\":{d}")).unwrap_or_default();
    // Answers appended before we send can't be ours: remember the file length
    // (always a line boundary) and scan only past it.
    let offset = fs::metadata(output_path()).map(|m| m.len() as usize).unwrap_or(0);
    let code = send(format!(
        "{{\"message\":\"{}\",\"actions\":\"{}\",\"id\":\"{}\",\"from\":\"{}\"{expires}}}\n",
        json_escape(&text),
        json_escape(&options),
        json_escape(&id),
        json_escape(&from)
    ));
    if code != 0 {
        return code;
    }
    // Block until the app appends our answer line to the output file.
    match wait_answer(&id, offset, deadline) {
        Some(answer) => {
            println!("{answer}");
            0
        }
        None => match flag(rest, "--default") {
            Some(default) => {
                println!("{default}");
                0
            }
            None => {
                eprintln!("{}", i18n::CLI_ASK_TIMEOUT);
                124
            }
        },
    }
}

fn remind(rest: &[String]) -> i32 {
    let (Some(text), Some(dur)) = (positional(rest), flag(rest, "--in")) else {
        return usage(i18n::CLI_USAGE_REMIND);
    };
    send(format!("{{\"remind\":\"{}\",\"in\":\"{}\"}}\n", json_escape(&text), json_escape(&dur)))
}

fn timer(rest: &[String]) -> i32 {
    let Some(dur) = positional(rest) else { return usage(i18n::CLI_USAGE_TIMER) };
    send(format!("{{\"timer\":\"{}\"}}\n", json_escape(&dur)))
}

fn action(rest: &[String]) -> i32 {
    let Some(a) = positional(rest) else { return usage(i18n::CLI_USAGE_DO) };
    send(format!("{{\"command\":\"{}\"}}\n", json_escape(&a)))
}

// `--from` is only recognized BEFORE the command so the watched command keeps
// its own flags intact: `tama watch --from ci cargo test --release`.
fn watch_parse(rest: &[String]) -> Option<(String, &[String])> {
    let (from, cmd_args) = match rest.first().map(String::as_str) {
        Some("--from") => (rest.get(1).cloned(), rest.get(2..).unwrap_or_default()),
        _ => (None, rest),
    };
    if cmd_args.is_empty() {
        return None;
    }
    Some((from.unwrap_or_else(|| cmd_args[0].clone()), cmd_args))
}

fn say_line(text: &str, kind: &str, from: &str) -> String {
    format!(
        "{{\"message\":\"{}\",\"type\":\"{}\",\"from\":\"{}\"}}\n",
        json_escape(text),
        json_escape(kind),
        json_escape(from)
    )
}

// Runs a command and reports its outcome to the pet: an info message when it
// starts, success/error by exit code when it ends. The notification is best
// effort — the command runs (and its exit code is propagated) even when the
// app is closed.
fn watch(rest: &[String]) -> i32 {
    let Some((from, cmd_args)) = watch_parse(rest) else { return usage(i18n::CLI_USAGE_WATCH) };
    let cmd = cmd_args.join(" "); // display only
    send(say_line(&i18n::msg_watch_start(&cmd), "info", &from));
    let started = std::time::Instant::now();
    // exec the args directly — joining into a shell string would lose quoting
    let status = std::process::Command::new(&cmd_args[0]).args(&cmd_args[1..]).status();
    let secs = started.elapsed().as_secs();
    let (text, kind, code) = match status {
        Ok(s) if s.success() => (i18n::msg_watch_ok(&cmd, secs), "success", 0),
        Ok(s) => {
            let code = s.code().unwrap_or(1);
            (i18n::msg_watch_fail(&cmd, code, secs), "error", code)
        }
        Err(e) => (format!("{cmd}: {e}"), "error", 127),
    };
    send(say_line(&text, kind, &from));
    code
}

fn pomodoro(rest: &[String]) -> i32 {
    let work = positional(rest).unwrap_or_else(|| "25m".to_string());
    if work == "off" {
        return send("{\"pomodoro\":\"off\"}\n".to_string());
    }
    let pause = flag(rest, "--break").unwrap_or_else(|| "5m".to_string());
    if crate::assistant::parse_duration(&work).is_none() || crate::assistant::parse_duration(&pause).is_none() {
        return usage(i18n::CLI_USAGE_POMODORO);
    }
    send(format!(
        "{{\"pomodoro\":\"{}\",\"break\":\"{}\"}}\n",
        json_escape(&work),
        json_escape(&pause)
    ))
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
    fn flags_collects_every_occurrence() {
        let rest = v(&["ok?", "--options", "permitir", "--from", "claude", "--options", "negar, talvez"]);
        assert_eq!(flags(&rest, "--options"), v(&["permitir", "negar, talvez"]));
        assert_eq!(flags(&rest, "--in"), Vec::<String>::new());
    }

    #[test]
    fn ask_options_single_flag_splits_commas_repeated_flags_are_literal() {
        assert_eq!(ask_options(&[]), "sim\nnão");
        assert_eq!(ask_options(&v(&["a, b,c"])), "a\nb\nc");
        assert_eq!(ask_options(&v(&["permitir", "Sim, e não pergunte de novo"])), "permitir\nSim, e não pergunte de novo");
    }

    #[test]
    fn unknown_command_falls_through_to_tui() {
        assert_eq!(run(&v(&["--gallery"])), None);
        assert_eq!(run(&[]), None);
    }

    #[test]
    fn watch_parse_keeps_the_commands_own_flags() {
        let rest = v(&["--from", "ci", "cargo", "test", "--release"]);
        let (from, cmd) = watch_parse(&rest).unwrap();
        assert_eq!(from, "ci");
        assert_eq!(cmd, &rest[2..]);
        // without --from, the origin defaults to the program name
        let rest = v(&["make", "-j4", "build"]);
        let (from, cmd) = watch_parse(&rest).unwrap();
        assert_eq!(from, "make");
        assert_eq!(cmd, &rest[..]);
        assert!(watch_parse(&[]).is_none());
        assert!(watch_parse(&v(&["--from", "ci"])).is_none());
    }
}
