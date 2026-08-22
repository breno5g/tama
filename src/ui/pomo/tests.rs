//! Pomodoro layout tests. The clock's position is the invariant under test: it
//! must stay horizontally centered and must not shift when the state changes.

use super::super::stats::progress_line;
use super::super::testutil::{named_pet, view_of, EMPTY_LOG};
use super::*;

// Same invariant as build_home: the pomodoro screen must fit the inner
// height at every terminal size, running or idle.
#[test]
fn build_pomo_fits_any_terminal_size() {
    let pet = named_pet();
    let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
    for iw in [10, 20, 30, 45, 60, 72, 80, 96, 120] {
        for ih in [1, 3, 5, 8, 12, 16, 20, 24, 28, 40] {
            for r in [None, Some(&run)] {
                let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, iw, ih);
                assert!(
                    c.len() <= ih.saturating_sub(1).max(1),
                    "overflow at {iw}x{ih} (run={}): {} lines",
                    r.is_some(),
                    c.len()
                );
            }
        }
    }
}

// The pet must be on the pomodoro screen at EVERY size that has room for
// more than the single status line: full art, or the tiny face fallback.
#[test]
fn pomo_screen_always_shows_the_pet() {
    let pet = named_pet();
    for (iw, ih) in [(96, 30), (96, 14), (60, 12), (40, 10), (30, 5)] {
        let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, iw, ih);
        let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
        assert!(text.contains("▄█▄") || text.contains("(=^"), "no pet at {iw}x{ih}");
    }
}

// Compact tiers must form ONE block: draw_screen centers lines
// independently, so any width variation would give the mascot, clock and
// presets each their own left edge.
#[test]
fn pomo_compact_tier_keeps_a_single_left_edge() {
    let pet = named_pet();
    let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
    for (iw, ih) in [(71, 20), (60, 14), (44, 12), (36, 10)] {
        for r in [None, Some(&run)] {
            let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, iw, ih);
            let w = c.iter().map(line_w).max().unwrap_or(0);
            assert!(
                c.iter().all(|l| line_w(l) == w),
                "ragged block at {iw}x{ih} (run={})",
                r.is_some()
            );
        }
    }
}

// (row, col) of the clock's top-left edge: the first run of 6 '█' (the
// top row of the first digit of "25:00").
fn clock_pos(c: &[Line]) -> Option<(usize, usize)> {
    for (y, l) in c.iter().enumerate() {
        let chars: Vec<char> = l.iter().flat_map(|(s, ..)| s.chars()).collect();
        if let Some(x) = chars.windows(6).position(|w| w.iter().all(|&ch| ch == '█')) {
            return Some((y, x));
        }
    }
    None
}

fn clock_x(c: &[Line]) -> Option<usize> {
    clock_pos(c).map(|(_, x)| x)
}

// The clock must sit at the exact same x whatever the state (idle,
// running, with tasks) and whatever art rung the height picks — and that
// x must be the horizontal center.
#[test]
fn pomo_clock_is_pinned_to_the_horizontal_center() {
    let pet = named_pet();
    let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 2 };
    let tasks = vec![progress_line("build", 40)];
    let states: [(Option<&PomoRun>, &Vec<Line>); 3] =
        [(None, &Vec::new()), (Some(&run), &Vec::new()), (Some(&run), &tasks)];
    let clock_w = 31; // big "25:00"
    // Full tier, several terminal widths (wider than the 96-col panel cap
    // too) and both art rungs (tall → large art, short → small art): the
    // pet+clock group must sit with EQUAL screen margins, and must not
    // move — in x OR y — when the state flips.
    for iw in [96, 100, 110] {
        for ih in [30, 26, 21] {
            let mut pos_seen = None;
            for (r, progress) in &states {
                let view = HomeView {
                    log: &EMPTY_LOG,
                    clock_text: "12:00",
                    hour: 12,
                    timer: None,
                    progress: (*progress).clone(),
                };
                let c = build_pomo(&pet, 0, &view, "25:00", *r, 0, iw, ih);
                let pos = clock_pos(&c).unwrap();
                assert_eq!(*pos_seen.get_or_insert(pos), pos, "clock moved at {iw}x{ih}");
                // screen coords: add draw_screen's centering pad
                let lw = c.iter().map(line_w).max().unwrap();
                let pad = (iw - lw) / 2;
                // the pet's left edge on screen, via the grass row (it
                // spans the pet block from its column 0)
                let left = c
                    .iter()
                    .find_map(|l| {
                        let chars: Vec<char> = l.iter().flat_map(|(s, ..)| s.chars()).collect();
                        chars.iter().position(|&ch| ch == '▁').map(|i| pad + i)
                    })
                    .unwrap();
                let right = iw - (pad + pos.1 + clock_w);
                assert!(
                    (left as i64 - right as i64).abs() <= 1,
                    "unbalanced at {iw}x{ih}: left {left}, right {right}"
                );
            }
        }
    }
    // With the tall art, the clock group must share the pet's vertical
    // center (offset down), not hug the top of the panel.
    let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, 96, 30);
    let (y, _) = clock_pos(&c).unwrap();
    assert!(y > 4, "clock is top-aligned against the pet: row {y}");
    // compact tier: the block is exactly clock-wide, so draw_screen's
    // centering lands the clock itself on the center.
    for (iw, ih) in [(70, 16), (50, 14), (40, 12)] {
        for (r, progress) in &states {
            let view = HomeView {
                log: &EMPTY_LOG,
                clock_text: "12:00",
                hour: 12,
                timer: None,
                progress: (*progress).clone(),
            };
            let c = build_pomo(&pet, 0, &view, "25:00", *r, 0, iw, ih);
            let w = c.iter().map(line_w).max().unwrap();
            assert_eq!(w, clock_w, "block wider than the clock at {iw}x{ih}");
            assert_eq!(clock_x(&c), Some(0));
        }
    }
}

// The compact tier's tiny face must not sit glued to the clock: the
// column's breathing row provides one blank line between them.
#[test]
fn pomo_compact_keeps_air_between_face_and_clock() {
    let pet = named_pet();
    let run = PomoRun { label: "foco", focus: true, frac: 40, cycle: 1 };
    for r in [None, Some(&run)] {
        let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", r, 0, 60, 16);
        let face_row =
            c.iter().position(|l| l.iter().any(|(s, ..)| s.contains("(=^"))).unwrap();
        let below: String = c[face_row + 1].iter().map(|(s, ..)| s.as_str()).collect();
        assert!(below.trim().is_empty(), "face glued to the clock: {below:?}");
    }
}

#[test]
fn pomo_full_tier_shows_clock_beside_pet_and_tasks_panel() {
    let pet = named_pet();
    let c = build_pomo(&pet, 0, &view_of(&EMPTY_LOG), "25:00", None, 0, 96, 30);
    let text: String = c.iter().flat_map(|l| l.iter()).map(|(s, ..)| s.as_str()).collect();
    assert!(text.contains(&format!("┌─ {}", i18n::t().pomo_title)));
    assert!(text.contains(&format!("┌─ {}", i18n::t().pomo_tasks)));
    assert!(text.contains(i18n::t().pomo_preset_labels[0]));
    assert!(text.contains("██")); // the big clock is there
}
