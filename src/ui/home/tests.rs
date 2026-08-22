//! Layout tests for the home screen: the height ladder, and above all that
//! nothing dynamic (event count, terminal height) reflows the frame.

use std::collections::VecDeque;

use super::super::testutil::{named_pet, view_of, EMPTY_LOG};
use super::*;

// The content must fit the inner height (minus the footer row) at EVERY
// terminal size — height responsiveness is exactly this invariant.
#[test]
fn build_home_fits_any_terminal_size() {
    let pet = named_pet();
    let log = VecDeque::new();
    for iw in [10, 20, 30, 45, 60, 72, 80, 96, 120] {
        for ih in [1, 3, 5, 8, 12, 16, 20, 24, 28, 40] {
            let c = build_home(&pet, 0, &view_of(&log), iw, ih);
            assert!(
                c.len() <= ih.saturating_sub(1).max(1),
                "overflow at {iw}x{ih}: {} lines",
                c.len()
            );
        }
    }
}

#[test]
fn tall_wide_terminal_gets_full_panel_layout() {
    let pet = named_pet();
    let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 96, 30);
    let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
    assert!(text.contains("┌─ rex"));
    assert!(text.contains(&format!("┌─ {}", i18n::t().panel_status)));
    assert!(text.contains(&format!("┌─ {}", i18n::t().panel_events)));
}

#[test]
fn short_wide_terminal_degrades_but_keeps_panels_when_possible() {
    let pet = named_pet();
    let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 96, 16);
    let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
    assert!(text.contains("┌─ rex"), "should still use the panel layout at 96x16");
    assert!(!text.contains(&format!("┌─ {}", i18n::t().panel_events)) || c.len() <= 15);
}

// A tall terminal must be USED, not just centered into: the leftover
// height flows into the pet scene (sky) and extra log entries.
#[test]
fn tall_terminal_fills_available_height() {
    let pet = named_pet();
    let mut log = VecDeque::new();
    for i in 0..8 {
        log.push_back(plain(format!("event {i}")));
    }
    let avail = 39; // 96x40 terminal
    let c = build_home(&pet, 0, &view_of(&log), 96, 40);
    assert!(c.len() >= avail - 4, "only {} of {avail} lines used", c.len());
    assert!(c.len() <= avail);
}

// The layout skeleton must not move as the log fills: same height and
// same row for every panel with 0, 1 or 12 events, at several sizes.
#[test]
fn event_count_never_resizes_the_layout() {
    let pet = named_pet();
    let row_of = |c: &[Line], needle: &str| {
        c.iter().position(|l| l.iter().any(|(s, ..)| s.contains(needle)))
    };
    for (iw, ih) in [(96, 40), (96, 30), (96, 24), (80, 18), (50, 16), (30, 12)] {
        let mut baseline: Option<(usize, Option<usize>, Option<usize>)> = None;
        for n in [0usize, 1, 12] {
            let mut log = VecDeque::new();
            for i in 0..n {
                log.push_back(plain(format!("event {i}")));
            }
            let c = build_home(&pet, 0, &view_of(&log), iw, ih);
            let shape = (c.len(), row_of(&c, "eventos"), row_of(&c, "▄█▄"));
            match &baseline {
                None => baseline = Some(shape),
                Some(b) => assert_eq!(*b, shape, "layout moved at {iw}x{ih} with {n} events"),
            }
        }
    }
}

#[test]
fn tiny_terminal_falls_back_to_face() {
    let pet = named_pet();
    let c = build_home(&pet, 0, &view_of(&EMPTY_LOG), 24, 4);
    let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
    assert!(text.contains("(=^"), "tiny face expected at 24x4");
}
