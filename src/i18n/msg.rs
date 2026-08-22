//! Messages built from `{placeholder}` templates.
//!
//! `format!` needs a literal, so a translatable message cannot be a format
//! string — the locale files hold named placeholders and `fill` substitutes
//! them. Callers keep the signatures they always had.

use super::t;
use crate::pet::Mood;
use crate::species::Species;

/// Substitutes `{key}` placeholders in one left-to-right pass.
///
/// One pass matters: chained `replace` calls would re-scan text that came from
/// a *value*, so an answer containing a literal `{text}` would be substituted
/// into. Unknown placeholders are left verbatim, which makes a typo in a
/// locale file visible on screen instead of silently blank.
fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}').map(|i| open + i) else { break };
        out.push_str(&rest[..open]);
        match args.iter().find(|(key, _)| *key == &rest[open + 1..close]) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(&rest[open..=close]),
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out
}

pub fn msg_played(name: &str) -> String {
    fill(t().msg_played, &[("name", name)])
}

pub fn msg_fed(food: &str, name: &str) -> String {
    fill(t().msg_fed, &[("food", food), ("name", name)])
}

pub fn msg_bathed(name: &str) -> String {
    fill(t().msg_bathed, &[("name", name)])
}

pub fn msg_sleep(name: &str, sleeping: bool) -> String {
    let template = if sleeping { t().msg_sleep } else { t().msg_wake };
    fill(template, &[("name", name)])
}

pub fn msg_zen(on: bool) -> String {
    if on { t().msg_zen_on } else { t().msg_zen_off }.to_string()
}

pub fn msg_became(name: &str, species: Species) -> String {
    fill(t().msg_became, &[("name", name), ("species", super::species_name(species))])
}

pub fn msg_level_up(name: &str, level: u32) -> String {
    fill(t().msg_level_up, &[("name", name), ("level", &level.to_string())])
}

pub fn msg_game(player: &str, pet: &str, outcome: &str) -> String {
    fill(t().msg_game, &[("player", player), ("pet", pet), ("outcome", outcome)])
}

pub fn msg_game_waiting(name: &str) -> String {
    fill(t().msg_game_waiting, &[("name", name)])
}

pub fn msg_celebrate(name: &str) -> String {
    fill(t().msg_celebrate, &[("name", name)])
}

pub fn msg_action_fed(name: &str) -> String {
    fill(t().msg_action_fed, &[("name", name)])
}

pub fn msg_reminder(text: &str) -> String {
    fill(t().msg_reminder, &[("text", text)])
}

pub fn msg_progress_done(from: &str) -> String {
    let from = if from.is_empty() { t().progress_fallback } else { from };
    fill(t().msg_progress_done, &[("from", from)])
}

pub fn msg_answered(text: &str, answer: &str) -> String {
    fill(t().msg_answered, &[("answer", answer), ("text", text)])
}

pub fn msg_ask_expired(from: &str) -> String {
    fill(t().msg_ask_expired, &[("from", from)])
}

/// `None` for the moods that raise no warning (empty template).
pub fn msg_warning(mood: Mood, name: &str) -> Option<String> {
    let template = t().msg_warnings[mood as usize];
    (!template.is_empty()).then(|| fill(template, &[("name", name)]))
}

fn fmt_secs(secs: u64) -> String {
    if secs >= 60 { format!("{}m{:02}s", secs / 60, secs % 60) } else { format!("{secs}s") }
}

pub fn msg_watch_start(cmd: &str) -> String {
    fill(t().msg_watch_start, &[("cmd", cmd)])
}

pub fn msg_watch_ok(cmd: &str, secs: u64) -> String {
    fill(t().msg_watch_ok, &[("cmd", cmd), ("secs", &fmt_secs(secs))])
}

pub fn msg_watch_fail(cmd: &str, code: i32, secs: u64) -> String {
    fill(
        t().msg_watch_fail,
        &[("cmd", cmd), ("code", &code.to_string()), ("secs", &fmt_secs(secs))],
    )
}

pub fn msg_http_on(addr: &str) -> String {
    fill(t().msg_http_on, &[("addr", addr)])
}

pub fn msg_http_fail(addr: &str, err: &str) -> String {
    fill(t().msg_http_fail, &[("addr", addr), ("err", err)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_substitutes_every_named_placeholder() {
        assert_eq!("a-b", fill("{x}-{y}", &[("x", "a"), ("y", "b")]));
        assert_eq!("hi tama!", fill("hi {name}!", &[("name", "tama")]));
    }

    #[test]
    fn fill_never_substitutes_into_a_value() {
        // the answer contains a literal placeholder: it must survive as text
        let out = fill("you said \"{answer}\" to: {text}", &[("answer", "{text}"), ("text", "why")]);
        assert_eq!("you said \"{text}\" to: why", out);
    }

    #[test]
    fn fill_keeps_unknown_placeholders_visible() {
        assert_eq!("{oops} here", fill("{oops} here", &[("name", "x")]));
    }

    #[test]
    fn fill_tolerates_unbalanced_braces() {
        assert_eq!("a {b", fill("a {b", &[("b", "!")]));
        assert_eq!("100% } done", fill("100% } done", &[]));
    }

    #[test]
    fn fmt_secs_switches_to_minutes_at_sixty() {
        assert_eq!("59s", fmt_secs(59));
        assert_eq!("1m00s", fmt_secs(60));
        assert_eq!("2m05s", fmt_secs(125));
    }

    #[test]
    fn warnings_exist_only_for_the_moods_that_need_one() {
        assert!(msg_warning(Mood::Happy, "tama").is_none());
        assert!(msg_warning(Mood::Sleeping, "tama").is_none());
        for m in [Mood::Hungry, Mood::Dirty, Mood::Sleepy, Mood::Sad] {
            assert!(msg_warning(m, "tama").is_some_and(|w| w.contains("tama")));
        }
    }
}
