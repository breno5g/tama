//! Assistant mode: external programs send one flat JSON object per line —
//! through the input pipe or an HTTP POST — and answers to questions go to
//! the output file, also as JSON lines. Invalid pipe lines are silently
//! ignored, per the design contract (HTTP gets a 400 instead).
//!
//! The schema is English (`message`, `from`, `actions`, ...); everything the
//! user READS stays in i18n.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::state::{data_dir, input_path, output_path};

// Protocol value, not UI text: what a discarded ask answers to its caller.
pub const ANSWER_IGNORED: &str = "ignored";
pub const ASK_POLL: Duration = Duration::from_millis(300);

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
    Ask { text: String, options: Vec<String>, id: String, from: String, expires: Option<u64> },
    Action(String),
    Progress { from: String, pct: u8 },
    Reminder { text: String, at: u64 },
    Timer { until: u64 },
    Pomodoro { work: u64, rest: u64 },
    PomodoroOff,
}

// Splits a flat JSON object into (key, value) pairs. Quote-aware, handles
// \" \\ \n \t \r escapes; an array of strings folds into ONE \n-separated
// value (how options travel internally). Deeper nesting is not part of the
// contract.
pub fn json_fields(line: &str) -> Option<Vec<(String, String)>> {
    let inner = line.trim().strip_prefix('{')?.strip_suffix('}')?;
    let mut fields = Vec::new();
    let mut token = String::new();
    let mut tokens: Vec<(String, bool)> = Vec::new(); // (text, came_from_string)
    let mut in_str = false;
    let mut in_arr = false;
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
        } else if in_arr {
            match c {
                '"' => in_str = true,
                ',' => token.push('\n'),
                ']' => {
                    in_arr = false;
                    was_str = true;
                }
                c if c.is_whitespace() => {}
                c => token.push(c),
            }
        } else {
            match c {
                '"' => in_str = true,
                '[' => in_arr = true,
                ':' | ',' => {
                    push(&mut token, &mut tokens, &mut was_str);
                    tokens.push((c.to_string(), false));
                }
                c if c.is_whitespace() => {}
                c => token.push(c),
            }
        }
    }
    if in_str || in_arr {
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

fn get(fields: &[(String, String)], k: &str) -> Option<String> {
    fields.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone())
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
        // `actions` turns a message into a question; without it, plain speech
        if let Some(actions) = get(fields, "actions") {
            let options: Vec<String> = actions
                .split('\n')
                .map(|o| o.trim().to_string())
                .filter(|o| !o.is_empty())
                .collect();
            let id = get(fields, "id").unwrap_or_else(|| format!("ask-{now}"));
            let expires = get(fields, "expires").and_then(|e| e.parse().ok());
            return Some(Msg::Ask {
                text,
                options: if options.is_empty() { vec!["sim".into(), "não".into()] } else { options },
                id,
                from,
                expires,
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

pub fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

pub fn answer_line(id: &str, answer: &str) -> String {
    format!("{{\"id\":\"{}\",\"answer\":\"{}\"}}\n", json_escape(id), json_escape(answer))
}

pub fn write_answer(id: &str, answer: &str) -> io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(output_path())?;
    f.write_all(answer_line(id, answer).as_bytes())
}

pub fn find_answer(content: &str, id: &str) -> Option<String> {
    for line in content.lines() {
        let Some(fields) = json_fields(line) else { continue };
        if get(&fields, "id").as_deref() == Some(id) {
            return Some(get(&fields, "answer").unwrap_or_default());
        }
    }
    None
}

// Polls the output file for the answer to `id`, scanning only past `offset`
// (record the file length BEFORE sending the ask). A shrunken file means the
// app restarted and truncated it: rescan everything. None once `deadline`
// (absolute epoch) passes without an answer.
pub fn wait_answer(id: &str, offset: usize, deadline: Option<u64>) -> Option<String> {
    loop {
        if let Ok(content) = std::fs::read_to_string(output_path()) {
            let tail = content.get(offset..).unwrap_or(&content);
            if let Some(answer) = find_answer(tail, id) {
                return Some(answer);
            }
        }
        if deadline.is_some_and(|d| now_epoch() >= d) {
            return None;
        }
        std::thread::sleep(ASK_POLL);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_fields_handles_colons_commas_and_escapes_inside_strings() {
        let fields = json_fields(r#"{"message":"deploy: ok, \"prod\"","from":"ci","progress":62}"#).unwrap();
        assert_eq!(fields[0], ("message".to_string(), r#"deploy: ok, "prod""#.to_string()));
        assert_eq!(fields[1], ("from".to_string(), "ci".to_string()));
        assert_eq!(fields[2], ("progress".to_string(), "62".to_string()));
    }

    #[test]
    fn json_fields_folds_string_arrays_into_one_value() {
        let fields = json_fields(r#"{"actions":["sim","não, depois","com \"aspas\""],"from":"x"}"#).unwrap();
        assert_eq!(fields[0], ("actions".to_string(), "sim\nnão, depois\ncom \"aspas\"".to_string()));
        assert_eq!(fields[1], ("from".to_string(), "x".to_string()));
        // empty array → empty value (parse falls back to default options)
        assert_eq!(json_fields(r#"{"actions":[]}"#).unwrap()[0].1, "");
        // unterminated array is invalid
        assert!(json_fields(r#"{"actions":["a","b"}"#).is_none());
    }

    #[test]
    fn invalid_lines_are_rejected() {
        assert!(json_fields("not json").is_none());
        assert!(json_fields(r#"{"unterminated":"x"#).is_none());
        assert!(parse_line("{}", 0).is_none());
        assert!(parse_line(r#"{"unknown":"x"}"#, 0).is_none());
        assert!(parse_msgs("garbage", 0).is_empty());
    }

    #[test]
    fn parse_line_covers_every_message_type() {
        let now = 1000;
        assert_eq!(
            parse_line(r#"{"message":"oi","type":"error","from":"ci"}"#, now),
            Some(Msg::Say { text: "oi".into(), from: "ci".into(), kind: Kind::Error })
        );
        assert_eq!(
            parse_line(r#"{"message":"subir?","actions":["sim","não"],"id":"rel-1"}"#, now),
            Some(Msg::Ask {
                text: "subir?".into(),
                options: vec!["sim".into(), "não".into()],
                id: "rel-1".into(),
                from: String::new(),
                expires: None
            })
        );
        assert_eq!(
            parse_line(r#"{"progress":62,"from":"backup"}"#, now),
            Some(Msg::Progress { from: "backup".into(), pct: 62 })
        );
        assert_eq!(
            parse_line(r#"{"remind":"standup","in":"10m"}"#, now),
            Some(Msg::Reminder { text: "standup".into(), at: now + 600 })
        );
        assert_eq!(parse_line(r#"{"timer":"25m"}"#, now), Some(Msg::Timer { until: now + 1500 }));
        assert_eq!(
            parse_line(r#"{"pomodoro":"25m","break":"5m"}"#, now),
            Some(Msg::Pomodoro { work: 1500, rest: 300 })
        );
        assert_eq!(parse_line(r#"{"pomodoro":"off"}"#, now), Some(Msg::PomodoroOff));
    }

    #[test]
    fn command_rides_alone_or_with_a_message() {
        assert_eq!(parse_msgs(r#"{"command":"celebrate"}"#, 0), vec![Msg::Action("celebrate".into())]);
        let both = parse_msgs(r#"{"command":"celebrate","message":"merge!","from":"ci"}"#, 0);
        assert_eq!(both.len(), 2);
        assert_eq!(both[0], Msg::Action("celebrate".into()));
        assert!(matches!(&both[1], Msg::Say { text, .. } if text == "merge!"));
    }

    #[test]
    fn pomodoro_defaults_break_to_5m_and_rejects_bad_durations() {
        assert_eq!(parse_line(r#"{"pomodoro":"25m"}"#, 0), Some(Msg::Pomodoro { work: 1500, rest: 300 }));
        assert!(parse_line(r#"{"pomodoro":"abc"}"#, 0).is_none());
        assert!(parse_line(r#"{"pomodoro":"25m","break":"abc"}"#, 0).is_none());
    }

    #[test]
    fn ask_defaults_options_and_id() {
        let Some(Msg::Ask { options, id, expires, .. }) =
            parse_line(r#"{"message":"ok?","actions":""}"#, 7)
        else {
            panic!("expected Ask");
        };
        assert_eq!(options, vec!["sim".to_string(), "não".to_string()]);
        assert_eq!(id, "ask-7");
        assert_eq!(expires, None);
    }

    #[test]
    fn escape_round_trips_newlines_tabs_quotes_and_backslashes() {
        let text = "rodar:\n\tnpm test \"tudo\" \\o/\r";
        let line = format!("{{\"message\":\"{}\"}}", json_escape(text));
        let fields = json_fields(&line).unwrap();
        assert_eq!(fields[0], ("message".to_string(), text.to_string()));
    }

    #[test]
    fn ask_options_may_contain_commas_and_expires_is_parsed() {
        let line = r#"{"message":"ok?","actions":["Sim, e não pergunte de novo","não"],"expires":1500}"#;
        let Some(Msg::Ask { options, expires, .. }) = parse_line(line, 1000) else {
            panic!("expected Ask");
        };
        assert_eq!(options, vec!["Sim, e não pergunte de novo".to_string(), "não".to_string()]);
        assert_eq!(expires, Some(1500));
        // non-numeric expires is ignored, not fatal
        let Some(Msg::Ask { expires, .. }) = parse_line(r#"{"message":"ok?","actions":"a","expires":"x"}"#, 0)
        else {
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
        assert_eq!(answer_line("a\"b", "sim"), "{\"id\":\"a\\\"b\",\"answer\":\"sim\"}\n");
    }

    #[test]
    fn find_answer_matches_id_and_skips_garbage() {
        let content = "lixo\n{\"id\":\"outro\",\"answer\":\"não\"}\n{\"id\":\"a-1\",\"answer\":\"sim\"}\n";
        assert_eq!(find_answer(content, "a-1"), Some("sim".to_string()));
        assert_eq!(find_answer(content, "a-2"), None);
        // scanning only the tail hides answers written before the offset
        let offset = content.find("{\"id\":\"a-1\"").unwrap();
        assert_eq!(find_answer(&content[offset..], "outro"), None);
    }

    #[test]
    fn wait_answer_gives_up_at_the_deadline() {
        // deadline already passed → returns None without blocking
        assert_eq!(wait_answer("no-such-id", usize::MAX, Some(0)), None);
    }

    #[test]
    fn progress_clamps_to_100() {
        assert_eq!(parse_line(r#"{"progress":250}"#, 0), Some(Msg::Progress { from: String::new(), pct: 100 }));
    }
}
