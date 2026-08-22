//! One function per subcommand. Each builds a JSON line and hands it to
//! `send`; `ask` additionally blocks on the answer and prints it to stdout.

use std::fs;

use super::{flag, flags, positional, send, usage};
use crate::assistant::{json_escape, now_epoch, parse_duration, wait_answer};
use crate::i18n;
use crate::state::output_path;

pub(super) fn say(rest: &[String]) -> i32 {
    let Some(text) = positional(rest) else { return usage(i18n::t().cli_usage_say) };
    let from = flag(rest, "--from").unwrap_or_default();
    let kind = flag(rest, "--type").unwrap_or_else(|| "info".to_string());
    send(say_line(&text, &kind, &from))
}

// Options travel as one JSON string with a literal \n between them: a single
// --options keeps the comma-split shorthand, repeated flags are literal (so
// an option may contain commas).
fn ask_options(opts: &[String]) -> String {
    match opts {
        [] => format!("{}\n{}", i18n::t().default_yes, i18n::t().default_no),
        [one] => one.split(',').map(str::trim).collect::<Vec<_>>().join("\n"),
        many => many.join("\n"),
    }
}

pub(super) fn ask(rest: &[String]) -> i32 {
    let Some(text) = positional(rest) else { return usage(i18n::t().cli_usage_ask) };
    // --input: free text is a valid answer; alone, the only one
    let typed = rest.iter().any(|a| a == "--input");
    let picks = flags(rest, "--options");
    let options = if typed && picks.is_empty() { String::new() } else { ask_options(&picks) };
    let from = flag(rest, "--from").unwrap_or_default();
    let id = flag(rest, "--id").unwrap_or_else(|| {
        format!("ask-{}-{}", now_epoch(), std::process::id())
    });
    let deadline = match flag(rest, "--timeout") {
        Some(t) => match parse_duration(&t) {
            Some(secs) => Some(now_epoch() + secs),
            None => return usage(i18n::t().cli_usage_ask),
        },
        None => None,
    };
    let expires = deadline.map(|d| format!(",\"expires\":{d}")).unwrap_or_default();
    let input = if typed { ",\"input\":true" } else { "" };
    // Answers appended before we send can't be ours: remember the file length
    // (always a line boundary) and scan only past it.
    let offset = fs::metadata(output_path()).map(|m| m.len() as usize).unwrap_or(0);
    let code = send(format!(
        "{{\"message\":\"{}\",\"actions\":\"{}\",\"id\":\"{}\",\"from\":\"{}\"{expires}{input}}}\n",
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
                eprintln!("{}", i18n::t().cli_ask_timeout);
                124
            }
        },
    }
}

pub(super) fn remind(rest: &[String]) -> i32 {
    let (Some(text), Some(dur)) = (positional(rest), flag(rest, "--in")) else {
        return usage(i18n::t().cli_usage_remind);
    };
    send(format!("{{\"remind\":\"{}\",\"in\":\"{}\"}}\n", json_escape(&text), json_escape(&dur)))
}

pub(super) fn timer(rest: &[String]) -> i32 {
    let Some(dur) = positional(rest) else { return usage(i18n::t().cli_usage_timer) };
    send(format!("{{\"timer\":\"{}\"}}\n", json_escape(&dur)))
}

pub(super) fn action(rest: &[String]) -> i32 {
    let Some(a) = positional(rest) else { return usage(i18n::t().cli_usage_do) };
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
pub(super) fn watch(rest: &[String]) -> i32 {
    let Some((from, cmd_args)) = watch_parse(rest) else { return usage(i18n::t().cli_usage_watch) };
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

pub(super) fn pomodoro(rest: &[String]) -> i32 {
    let work = positional(rest).unwrap_or_else(|| "25m".to_string());
    if work == "off" {
        return send("{\"pomodoro\":\"off\"}\n".to_string());
    }
    let pause = flag(rest, "--break").unwrap_or_else(|| "5m".to_string());
    if parse_duration(&work).is_none() || parse_duration(&pause).is_none() {
        return usage(i18n::t().cli_usage_pomodoro);
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
    fn ask_options_single_flag_splits_commas_repeated_flags_are_literal() {
        assert_eq!(ask_options(&[]), format!("{}\n{}", i18n::t().default_yes, i18n::t().default_no));
        assert_eq!(ask_options(&v(&["a, b,c"])), "a\nb\nc");
        assert_eq!(ask_options(&v(&["permitir", "Sim, e não pergunte de novo"])), "permitir\nSim, e não pergunte de novo");
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
