//! Message-parsing tests: what each wire shape turns into.

use super::*;
use crate::i18n;

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
            expires: None,
            input: false
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
fn input_true_makes_a_text_only_ask_and_coexists_with_actions() {
    // input alone: an Ask with NO options (text-only)
    let Some(Msg::Ask { options, input, .. }) = parse_line(r#"{"message":"como faço?","input":true}"#, 0)
    else {
        panic!("expected Ask");
    };
    assert!(options.is_empty());
    assert!(input);
    // input + actions: options preserved, typing is one more choice
    let Some(Msg::Ask { options, input, .. }) =
        parse_line(r#"{"message":"qual?","actions":["a","b"],"input":true}"#, 0)
    else {
        panic!("expected Ask");
    };
    assert_eq!(options, vec!["a".to_string(), "b".to_string()]);
    assert!(input);
    // without input, empty actions still fall back to the defaults
    let Some(Msg::Ask { options, input, .. }) = parse_line(r#"{"message":"ok?","actions":""}"#, 0) else {
        panic!("expected Ask");
    };
    assert_eq!(options.len(), 2);
    assert!(!input);
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
    assert_eq!(options, vec![i18n::t().default_yes, i18n::t().default_no]);
    assert_eq!(id, "ask-7");
    assert_eq!(expires, None);
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
fn progress_clamps_to_100() {
    assert_eq!(parse_line(r#"{"progress":250}"#, 0), Some(Msg::Progress { from: String::new(), pct: 100 }));
}
