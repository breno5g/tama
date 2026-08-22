//! Assistant mode: external programs write one flat JSON object per line to
//! the input pipe; answers to questions go to the output file, also as JSON
//! lines. Invalid lines are silently ignored, per the design contract.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::{data_dir, input_path, output_path};

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
            "sucesso" => Kind::Success,
            "alerta" => Kind::Warn,
            "erro" => Kind::Error,
            _ => Kind::Info,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    Say { text: String, from: String, kind: Kind },
    Ask { text: String, options: Vec<String>, id: String, from: String, expires: Option<u64> },
    Action(String),
    Progress { from: String, pct: u8 },
    Reminder { text: String, at: u64 },
    Timer { until: u64 },
    Pomodoro { work: u64, rest: u64 },
    PomodoroOff,
}

// Splits a flat JSON object into (key, value) pairs. Quote-aware, handles
// \" and \\ escapes; nested structures are not part of the contract.
pub fn json_fields(line: &str) -> Option<Vec<(String, String)>> {
    let inner = line.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut fields = Vec::new();
    let mut token = String::new();
    let mut tokens: Vec<(String, bool)> = Vec::new(); // (text, came_from_string)
    let mut in_str = false;
    let mut was_str = false;
    let mut esc = false;
    let push = |t: &mut String, tokens: &mut Vec<(String, bool)>, was_str: &mut bool| {
        tokens.push((std::mem::take(t), *was_str));
        *was_str = false;
    };
    for c in inner.chars() {
        if in_str {
            if esc {
                token.push(match c {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    c => c,
                });
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == '"' {
                in_str = false;
                was_str = true;
            } else {
                token.push(c);
            }
        } else {
            match c {
                '"' => in_str = true,
                ':' | ',' => {
                    push(&mut token, &mut tokens, &mut was_str);
                    tokens.push((c.to_string(), false));
                }
                c if c.is_whitespace() => {}
                c => token.push(c),
            }
        }
    }
    if in_str {
        return None;
    }
    push(&mut token, &mut tokens, &mut was_str);

    // tokens now look like: key, ":", value, ",", key, ":", value, …
    let mut it = tokens.into_iter().filter(|(t, s)| *s || !t.is_empty());
    loop {
        let Some((key, _)) = it.next() else { break };
        if key == "," {
            continue;
        }
        if it.next().map(|(t, _)| t) != Some(":".to_string()) {
            return None;
        }
        let (value, _) = it.next()?;
        if value == "," || value == ":" {
            return None;
        }
        fields.push((key, value));
    }
    Some(fields)
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

pub fn parse_line(line: &str, now: u64) -> Option<Msg> {
    let fields = json_fields(line)?;
    let get = |k: &str| fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
    let from = get("de").unwrap_or_default();
    if let Some(text) = get("fala") {
        return Some(Msg::Say { text, from, kind: Kind::from_id(&get("tipo").unwrap_or_default()) });
    }
    if let Some(text) = get("pergunta") {
        let options: Vec<String> = get("opcoes")
            .unwrap_or_else(|| "sim\nnão".to_string())
            .split('\n')
            .map(|o| o.trim().to_string())
            .filter(|o| !o.is_empty())
            .collect();
        let id = get("id").unwrap_or_else(|| format!("ask-{now}"));
        let expires = get("expira").and_then(|e| e.parse().ok());
        return Some(Msg::Ask { text, options: if options.is_empty() { vec!["sim".into(), "não".into()] } else { options }, id, from, expires });
    }
    if let Some(a) = get("acao") {
        return Some(Msg::Action(a));
    }
    if let Some(p) = get("progresso") {
        return Some(Msg::Progress { from, pct: p.parse::<u16>().ok()?.min(100) as u8 });
    }
    if let Some(text) = get("lembrete") {
        return Some(Msg::Reminder { text, at: now + parse_duration(&get("em")?)? });
    }
    if let Some(t) = get("timer") {
        return Some(Msg::Timer { until: now + parse_duration(&t)? });
    }
    if let Some(p) = get("pomodoro") {
        if p == "off" || p == "parar" {
            return Some(Msg::PomodoroOff);
        }
        let rest = get("pausa").map_or(Some(300), |s| parse_duration(&s))?;
        return Some(Msg::Pomodoro { work: parse_duration(&p)?, rest });
    }
    None
}

pub fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

pub fn answer_line(id: &str, answer: &str) -> String {
    format!("{{\"id\":\"{}\",\"resposta\":\"{}\"}}\n", json_escape(id), json_escape(answer))
}

pub fn write_answer(id: &str, answer: &str) -> io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(output_path())?;
    f.write_all(answer_line(id, answer).as_bytes())
}

// Ensures the input FIFO exists and streams its lines through a channel.
// The reader thread blocks on open/read (a FIFO with no writer blocks), so
// the main loop stays non-blocking via try_recv.
pub fn spawn_reader() -> Receiver<String> {
    let path: PathBuf = input_path();
    let _ = std::fs::create_dir_all(data_dir());
    if !path.exists() {
        let _ = std::process::Command::new("mkfifo").arg(&path).status();
    }
    let (tx, rx) = channel();
    std::thread::spawn(move || loop {
        let Ok(f) = File::open(&path) else { return };
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
        // EOF: every writer closed; reopen and keep listening.
    });
    rx
}

pub fn now_epoch() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_fields_handles_colons_commas_and_escapes_inside_strings() {
        let fields = json_fields(r#"{"fala":"deploy: ok, \"prod\"","de":"ci","progresso":62}"#).unwrap();
        assert_eq!(fields[0], ("fala".to_string(), r#"deploy: ok, "prod""#.to_string()));
        assert_eq!(fields[1], ("de".to_string(), "ci".to_string()));
        assert_eq!(fields[2], ("progresso".to_string(), "62".to_string()));
    }

    #[test]
    fn invalid_lines_are_rejected() {
        assert!(json_fields("not json").is_none());
        assert!(json_fields(r#"{"unterminated":"x"#).is_none());
        assert!(parse_line("{}", 0).is_none());
        assert!(parse_line(r#"{"desconhecido":"x"}"#, 0).is_none());
    }

    #[test]
    fn parse_line_covers_every_message_type() {
        let now = 1000;
        assert_eq!(
            parse_line(r#"{"fala":"oi","tipo":"erro","de":"ci"}"#, now),
            Some(Msg::Say { text: "oi".into(), from: "ci".into(), kind: Kind::Error })
        );
        assert_eq!(
            parse_line(r#"{"pergunta":"subir?","opcoes":"sim\nnão","id":"rel-1"}"#, now),
            Some(Msg::Ask {
                text: "subir?".into(),
                options: vec!["sim".into(), "não".into()],
                id: "rel-1".into(),
                from: String::new(),
                expires: None
            })
        );
        assert_eq!(parse_line(r#"{"acao":"comemorar"}"#, now), Some(Msg::Action("comemorar".into())));
        assert_eq!(
            parse_line(r#"{"progresso":62,"de":"backup"}"#, now),
            Some(Msg::Progress { from: "backup".into(), pct: 62 })
        );
        assert_eq!(
            parse_line(r#"{"lembrete":"standup","em":"10m"}"#, now),
            Some(Msg::Reminder { text: "standup".into(), at: now + 600 })
        );
        assert_eq!(parse_line(r#"{"timer":"25m"}"#, now), Some(Msg::Timer { until: now + 1500 }));
        assert_eq!(
            parse_line(r#"{"pomodoro":"25m","pausa":"5m"}"#, now),
            Some(Msg::Pomodoro { work: 1500, rest: 300 })
        );
        assert_eq!(parse_line(r#"{"pomodoro":"off"}"#, now), Some(Msg::PomodoroOff));
        assert_eq!(parse_line(r#"{"pomodoro":"parar"}"#, now), Some(Msg::PomodoroOff));
    }

    #[test]
    fn pomodoro_defaults_break_to_5m_and_rejects_bad_durations() {
        assert_eq!(parse_line(r#"{"pomodoro":"25m"}"#, 0), Some(Msg::Pomodoro { work: 1500, rest: 300 }));
        assert!(parse_line(r#"{"pomodoro":"abc"}"#, 0).is_none());
        assert!(parse_line(r#"{"pomodoro":"25m","pausa":"abc"}"#, 0).is_none());
    }

    #[test]
    fn ask_defaults_options_and_id() {
        let Some(Msg::Ask { options, id, expires, .. }) = parse_line(r#"{"pergunta":"ok?"}"#, 7) else {
            panic!("expected Ask");
        };
        assert_eq!(options, vec!["sim".to_string(), "não".to_string()]);
        assert_eq!(id, "ask-7");
        assert_eq!(expires, None);
    }

    #[test]
    fn escape_round_trips_newlines_tabs_quotes_and_backslashes() {
        let text = "rodar:\n\tnpm test \"tudo\" \\o/\r";
        let line = format!("{{\"fala\":\"{}\"}}", json_escape(text));
        let fields = json_fields(&line).unwrap();
        assert_eq!(fields[0], ("fala".to_string(), text.to_string()));
    }

    #[test]
    fn ask_options_may_contain_commas_and_expira_is_parsed() {
        let line = r#"{"pergunta":"ok?","opcoes":"Sim, e não pergunte de novo\nnão","expira":1500}"#;
        let Some(Msg::Ask { options, expires, .. }) = parse_line(line, 1000) else {
            panic!("expected Ask");
        };
        assert_eq!(options, vec!["Sim, e não pergunte de novo".to_string(), "não".to_string()]);
        assert_eq!(expires, Some(1500));
        // expira que não é número → ignorada, não derruba a mensagem
        let Some(Msg::Ask { expires, .. }) = parse_line(r#"{"pergunta":"ok?","expira":"x"}"#, 0) else {
            panic!("expected Ask");
        };
        assert_eq!(expires, None);
    }

    #[test]
    fn durations_parse() {
        assert_eq!(parse_duration("30s"), Some(30));
        assert_eq!(parse_duration("10m"), Some(600));
        assert_eq!(parse_duration("2h"), Some(7200));
        assert_eq!(parse_duration("abc"), None);
        assert_eq!(parse_duration(""), None);
    }

    #[test]
    fn answer_line_escapes() {
        assert_eq!(answer_line("a\"b", "sim"), "{\"id\":\"a\\\"b\",\"resposta\":\"sim\"}\n");
    }

    #[test]
    fn progress_clamps_to_100() {
        assert_eq!(parse_line(r#"{"progresso":250}"#, 0), Some(Msg::Progress { from: String::new(), pct: 100 }));
    }
}
