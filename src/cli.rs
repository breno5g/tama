//! CLI subcommands for external integration: they only write JSON lines to
//! the input pipe (`tama ask` additionally waits for its answer line in the
//! output file and prints it to stdout).

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::mpsc::channel;
use std::time::Duration;

use crate::assistant::{json_escape, json_fields, now_epoch};
use crate::i18n;
use crate::state::{input_path, output_path};

const PIPE_OPEN_TIMEOUT: Duration = Duration::from_secs(2);
const ASK_POLL: Duration = Duration::from_millis(300);

// Returns Some(exit code) when args named a subcommand; None → run the TUI.
pub fn run(args: &[String]) -> Option<i32> {
    let cmd = args.first()?.as_str();
    let rest = &args[1..];
    match cmd {
        "say" => Some(say(rest)),
        "ask" => Some(ask(rest)),
        "lembrar" => Some(remind(rest)),
        "timer" => Some(timer(rest)),
        "do" => Some(action(rest)),
        _ => None,
    }
}

fn flag(rest: &[String], name: &str) -> Option<String> {
    rest.iter().position(|a| a == name).and_then(|i| rest.get(i + 1)).cloned()
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
    let from = flag(rest, "--de").unwrap_or_default();
    let kind = flag(rest, "--tipo").unwrap_or_else(|| "info".to_string());
    send(format!(
        "{{\"fala\":\"{}\",\"tipo\":\"{}\",\"de\":\"{}\"}}\n",
        json_escape(&text),
        json_escape(&kind),
        json_escape(&from)
    ))
}

fn ask(rest: &[String]) -> i32 {
    let Some(text) = positional(rest) else { return usage(i18n::CLI_USAGE_ASK) };
    let options = flag(rest, "--opcoes").unwrap_or_else(|| "sim,não".to_string());
    let from = flag(rest, "--de").unwrap_or_default();
    let id = flag(rest, "--id").unwrap_or_else(|| {
        format!("ask-{}-{}", now_epoch(), std::process::id())
    });
    let code = send(format!(
        "{{\"pergunta\":\"{}\",\"opcoes\":\"{}\",\"id\":\"{}\",\"de\":\"{}\"}}\n",
        json_escape(&text),
        json_escape(&options),
        json_escape(&id),
        json_escape(&from)
    ));
    if code != 0 {
        return code;
    }
    // Block until the app appends our answer line to the output file.
    loop {
        if let Ok(content) = fs::read_to_string(output_path()) {
            for line in content.lines() {
                let Some(fields) = json_fields(line) else { continue };
                let get = |k: &str| fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
                if get("id") == Some(&id) {
                    println!("{}", get("resposta").unwrap_or_default());
                    return 0;
                }
            }
        }
        std::thread::sleep(ASK_POLL);
    }
}

fn remind(rest: &[String]) -> i32 {
    let (Some(text), Some(dur)) = (positional(rest), flag(rest, "--em")) else {
        return usage(i18n::CLI_USAGE_REMIND);
    };
    send(format!("{{\"lembrete\":\"{}\",\"em\":\"{}\"}}\n", json_escape(&text), json_escape(&dur)))
}

fn timer(rest: &[String]) -> i32 {
    let Some(dur) = positional(rest) else { return usage(i18n::CLI_USAGE_TIMER) };
    send(format!("{{\"timer\":\"{}\"}}\n", json_escape(&dur)))
}

fn action(rest: &[String]) -> i32 {
    let Some(a) = positional(rest) else { return usage(i18n::CLI_USAGE_DO) };
    send(format!("{{\"acao\":\"{}\"}}\n", json_escape(&a)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn flags_and_positionals_parse_in_any_order() {
        let rest = v(&["--de", "ci", "build ok", "--tipo", "sucesso"]);
        assert_eq!(positional(&rest), Some("build ok".to_string()));
        assert_eq!(flag(&rest, "--de"), Some("ci".to_string()));
        assert_eq!(flag(&rest, "--tipo"), Some("sucesso".to_string()));
        assert_eq!(flag(&rest, "--em"), None);
    }

    #[test]
    fn unknown_command_falls_through_to_tui() {
        assert_eq!(run(&v(&["--gallery"])), None);
        assert_eq!(run(&[]), None);
    }
}
