//! Assistant-screen tests. The recurring invariant: the card's size follows the
//! message SHAPE (ask vs say), never its content length or option count.

use super::super::answer::option_labels;
use super::super::testutil::{named_pet, sample_ask, view_of, EMPTY_LOG};
use super::*;

#[test]
fn free_text_is_listed_as_the_last_numbered_option() {
    let opts: Vec<String> = vec!["a".into(), "b".into()];
    assert_eq!(option_labels(&opts, false), opts);
    let with = option_labels(&opts, true);
    assert_eq!(with.len(), 3);
    assert_eq!(with[2], i18n::t().option_write);
    // no free key left (9 options) → no extra entry, `t` still opens it
    let nine: Vec<String> = (0..9).map(|i| i.to_string()).collect();
    assert_eq!(option_labels(&nine, true).len(), 9);
    // it renders in the card
    let pet = named_pet();
    let mut m = sample_ask("qual?", &opts, None);
    m.input_ok = true;
    let c = build_assistant(&pet, 0, Some(&m), &[], 0, &view_of(&EMPTY_LOG), 96, 24);
    let text: String = c.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
    assert!(text.contains(i18n::t().option_write), "extra option missing: {text}");
}

#[test]
fn typing_replaces_the_options_without_resizing_the_card() {
    let pet = named_pet();
    let options: Vec<String> = vec!["sim".into(), "não".into()];
    for (iw, ih) in [(96, 24), (80, 18), (50, 16), (26, 8)] {
        let idle = sample_ask("responde?", &options, None);
        let mut typing = sample_ask("responde?", &options, None);
        typing.input = Some("uma resposta escrita");
        let a = build_assistant(&pet, 0, Some(&idle), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
        let b = build_assistant(&pet, 0, Some(&typing), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
        assert_eq!(a.len(), b.len(), "card resized while typing at {iw}x{ih}");
        let text: String = b.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
        assert!(text.contains("resposta escrita"), "typed text missing at {iw}x{ih}");
    }
}

#[test]
fn text_only_ask_has_no_options_but_still_fits() {
    let pet = named_pet();
    let empty: Vec<String> = Vec::new();
    let mut m = sample_ask("o que você acha?", &empty, Some(30));
    m.input = Some("porque sim");
    for (iw, ih) in [(96, 24), (60, 12), (26, 8)] {
        let c = build_assistant(&pet, 0, Some(&m), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
        assert!(c.len() <= ih.saturating_sub(1).max(1), "overflow at {iw}x{ih}");
        let text: String = c.iter().flat_map(|l| l.iter().map(|(s, ..)| s.clone())).collect();
        assert!(text.contains("porque sim"), "typed text missing at {iw}x{ih}");
    }
}

#[test]
fn build_assistant_fits_any_terminal_size() {
    let pet = named_pet();
    let long_text = "claude quer executar: npm run build && rm -rf dist && cp x y ".repeat(5);
    let options: Vec<String> = vec![
        "permitir".into(),
        "Sim, e não pergunte de novo nesta sessão inteira por favor".into(),
        "negar".into(),
        "decidir no claude".into(),
    ];
    let queue = vec!["ci: build ok".to_string()];
    for iw in [10, 20, 26, 30, 45, 60, 72, 80, 96, 120] {
        for ih in [1, 3, 5, 6, 8, 12, 16, 20, 24, 28, 40] {
            for msg in [
                None,
                Some(sample_ask(&long_text, &options, Some(59))),
                Some(AssistantMsg {
                    text: "oi",
                    from: "ci",
                    kind: Kind::Success,
                    kind_label: "sucesso",
                    options: None,
                    expires_in: None,
                    input: None,
                    input_ok: false,
                    sel: 0,
                }),
            ] {
                let c = build_assistant(&pet, 0, msg.as_ref(), &queue, 1, &view_of(&EMPTY_LOG), iw, ih);
                assert!(
                    c.len() <= ih.saturating_sub(1).max(1),
                    "overflow at {iw}x{ih}: {} lines",
                    c.len()
                );
            }
        }
    }
}

#[test]
fn option_count_never_resizes_the_ask_bubble() {
    let pet = named_pet();
    for (iw, ih) in [(96, 24), (80, 18), (50, 16), (26, 8)] {
        let mut baseline: Option<usize> = None;
        for n in [1usize, 3, 9] {
            let options: Vec<String> = (0..n).map(|i| format!("opção {i}")).collect();
            let msg = sample_ask("posso?", &options, None);
            let c = build_assistant(&pet, 0, Some(&msg), &[], 0, &view_of(&EMPTY_LOG), iw, ih);
            match &baseline {
                None => baseline = Some(c.len()),
                Some(b) => assert_eq!(*b, c.len(), "bubble resized at {iw}x{ih} with {n} options"),
            }
        }
    }
}
