//! The main loop and the state it owns.
//!
//! Every phase of a frame is a method on `App`: drain the inbox, fire what is
//! due, draw, handle a key. They are methods rather than free functions because
//! the loop shares a dozen mutable pieces of state — threading those through
//! parameters was what made the original single function unreadable.
//!
//! `out` is deliberately NOT a field: `view()` borrows `&self`, and the writer
//! is borrowed mutably to draw, so keeping them separate avoids a borrow
//! conflict on every frame.

mod actions;
mod inbox;
mod keys;
mod keys_assistant;
mod messages;
mod pomo;
mod render;
mod screen;
mod setup;

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use crossterm::event::{self, Event, KeyEventKind};
use crossterm::style::Color;

use crate::assistant::{self, Msg};
use crate::pet::{Mood, Pet, DECAY_SECS};
use crate::state::{self, save};
use crate::ui::{seg, tinted, Clock, Line};

use inbox::{notify_android, notify_clear, Inbox};
use pomo::Pomo;
use screen::Screen;

const LOG_CAP: usize = 12;
const SAY_SECS: u64 = 8;
const ANSWER_CAP: usize = 1000; // typed answer: room for a paragraph, still bounded

pub struct App<'a> {
    pet: &'a mut Pet,
    rx: Receiver<String>,
    clock: Clock,
    log: VecDeque<Line>,
    inbox: Inbox,
    progress: Vec<(String, u8)>,
    reminders: Vec<(String, u64)>,
    timer_until: Option<u64>,
    pomo: Option<Pomo>,
    schedule_dirty: bool,
    notified: bool, // an Android notification is currently showing
    screen: Screen,
    // where an Ask yanked the user from; restored when the inbox drains
    prev_screen: Screen,
    ask_sel: usize,     // highlighted option of the question on screen
    ask_id: String,     // which question that state belongs to
    input: Option<String>, // Some = the current ask is being answered by text
    frame: usize,
    ticks_250ms: u64,
    prev_mood: Mood,
    // refreshed once per frame, so no phase has to pass them around
    time: String,
    hour: u8,
    now: u64,
}

impl<'a> App<'a> {
    fn new(pet: &'a mut Pet, rx: Receiver<String>) -> Self {
        // reminders, timer and pomodoro outlive the process: Android kills
        // Termux whenever it feels like it, and "remind me in 10min" must
        // survive that
        let saved = state::load_schedule(assistant::now_epoch());
        let mut clock = Clock::new();
        let (time, hour) = clock.get();
        let prev_mood = pet.mood();
        App {
            pet,
            rx,
            clock,
            log: VecDeque::new(),
            inbox: Inbox::new(),
            progress: Vec::new(),
            reminders: saved.reminders,
            timer_until: saved.timer,
            pomo: saved.pomo.map(Pomo::from),
            schedule_dirty: false,
            notified: false,
            screen: Screen::Home,
            prev_screen: Screen::Home,
            ask_sel: 0,
            ask_id: String::new(),
            input: None,
            frame: 0,
            ticks_250ms: 0,
            prev_mood,
            time,
            hour,
            now: assistant::now_epoch(),
        }
    }

    fn log_line(&mut self, line: Line) {
        self.log.push_back(line);
        while self.log.len() > LOG_CAP {
            self.log.pop_front();
        }
    }

    // A log entry stamped with the current clock, optionally with a colored
    // suffix like "(+10 xp)".
    fn log_at(&mut self, text: String, suffix: Option<(String, Color)>) {
        let mut l: Line = vec![seg(format!("{} ", self.time), Some(Color::DarkGrey)), seg(text, None)];
        if let Some((s, c)) = suffix {
            l.push(seg(format!(" {s}"), Some(c)));
        }
        self.log_line(l);
    }

    // Animation clock, stat decay and the mood-change warning: what happens on
    // a frame where nobody pressed anything.
    fn tick(&mut self) {
        self.ticks_250ms += 1;
        if self.ticks_250ms % 2 == 0 {
            self.frame += 1; // animation clock: ~500ms
        }
        // ponytail: sleeping recovers on a fast clock (2s), normal decay on DECAY_SECS
        if self.pet.sleeping && self.ticks_250ms % 8 == 0 {
            self.pet.tick();
        } else if self.ticks_250ms % (DECAY_SECS * 4) == 0 {
            self.pet.tick();
        }
        let mood = self.pet.mood();
        if mood != self.prev_mood {
            if let Some(w) = crate::i18n::msg_warning(mood, &self.pet.name) {
                let line =
                    vec![seg(format!("{} ", self.time), Some(Color::DarkGrey)), seg(w, Some(crate::ui::mood_color(mood)))];
                self.log_line(line);
            }
            self.prev_mood = mood;
        }
    }

    // Leaving for good: discarded questions must be answered so no script hangs,
    // and the notification must not outlive the app.
    fn quit(&mut self) {
        self.inbox.clear();
        notify_clear(self.notified);
    }
}

pub fn run(out: &mut impl Write, pet: &mut Pet, is_new: bool) -> io::Result<()> {
    if is_new {
        pet.species = setup::pick_species(out, pet.species)?;
        save(pet)?;
    }
    if pet.name.is_empty() {
        pet.name = setup::ask_name(out, pet.species)?;
        save(pet)?;
    }

    // FIFO and HTTP feed the same channel; the loop drains one stream.
    let (tx, rx) = std::sync::mpsc::channel();
    assistant::spawn_reader(tx.clone());
    let http_status = crate::http::spawn_http(tx);
    // Stale answers would satisfy the wrong ask; a fresh session starts clean.
    // ponytail: races a CLI still polling from a previous session; accepted
    let _ = std::fs::write(state::output_path(), "");

    let mut app = App::new(pet, rx);
    app.log_line(tinted(http_status, Color::DarkGrey));

    loop {
        let (time, hour) = app.clock.get();
        app.time = time;
        app.hour = hour;
        app.now = assistant::now_epoch();

        app.drain(out)?;
        app.fire_due()?;
        app.purge_expired();
        app.promote_assistant();
        app.flush_schedule();
        app.sync_notification();
        app.draw(out)?;

        if event::poll(Duration::from_millis(250))? {
            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            if app.on_key(out, k)? {
                return Ok(());
            }
        } else {
            app.tick();
        }
    }
}
