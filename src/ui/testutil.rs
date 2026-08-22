//! Fixtures shared by the layout tests. They live here rather than in each
//! test module because four of them build the same pet, view and sample ask.

use std::collections::VecDeque;

use super::{AssistantMsg, HomeView, Line};
use crate::assistant::Kind;
use crate::pet::Pet;

pub static EMPTY_LOG: VecDeque<Line> = VecDeque::new();

pub fn view_of(log: &VecDeque<Line>) -> HomeView<'_> {
    HomeView { log, clock_text: "12:00", hour: 12, timer: None, progress: Vec::new() }
}

pub fn sample_ask<'a>(text: &'a str, options: &'a [String], expires_in: Option<u64>) -> AssistantMsg<'a> {
    AssistantMsg {
        text,
        from: "claude",
        kind: Kind::Info,
        kind_label: "info",
        options: Some(options),
        expires_in,
        input: None,
        input_ok: false,
        sel: 0,
    }
}

pub fn named_pet() -> Pet {
    Pet { name: "rex".into(), ..Pet::default() }
}
