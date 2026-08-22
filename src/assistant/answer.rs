//! Answers travelling back to whoever asked: appended as JSON lines to the
//! output file, which the CLI and the HTTP handler poll for their own id.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::time::Duration;

use super::json::{get, json_escape, json_fields};
use super::now_epoch;
use crate::state::output_path;

pub const ASK_POLL: Duration = Duration::from_millis(300);

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
