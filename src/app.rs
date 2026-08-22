use std::collections::VecDeque;
use std::io::{self, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::Color;

use crate::assistant::{self, Kind, Msg};
use crate::i18n;
use crate::pet::{adj, Mood, Pet, DECAY_SECS, FOODS};
use crate::species::{render_art, render_tiny, ArtSize, Species, SPECIES};
use crate::state::{self, save};
use crate::ui::{self, draw_screen, plain, seg, tinted, Clock, HomeView, Line};

const LOG_CAP: usize = 12;
const SAY_SECS: u64 = 8;
const ANSWER_CAP: usize = 1000; // typed answer: room for a paragraph, still bounded

#[derive(Clone, Copy)]
enum Screen {
    Home,
    Actions(usize),
    Menu(usize),
    Game,
    Assistant,
    Pomo(usize),
}

// Index-aligned with i18n::ACTION_LABELS and ui::ACTION_GLYPHS.
#[derive(Clone, Copy, PartialEq)]
enum Action {
    Feed,
    Play,
    Sleep,
    Bath,
    Game,
    Assistant,
    Pomo,
    Zen,
    Switch,
}

const ACTIONS_ALL: [Action; 9] = [
    Action::Feed,
    Action::Play,
    Action::Sleep,
    Action::Bath,
    Action::Game,
    Action::Assistant,
    Action::Pomo,
    Action::Zen,
    Action::Switch,
];
const ACTIONS_ZEN: [Action; 4] = [Action::Assistant, Action::Pomo, Action::Zen, Action::Switch];

// Index-aligned with i18n::POMO_PRESET_LABELS: (focus, break) in seconds.
const POMO_PRESETS: [(u64, u64); 3] = [(25 * 60, 5 * 60), (50 * 60, 10 * 60), (15 * 60, 3 * 60)];

fn actions_for(zen: bool) -> &'static [Action] {
    if zen { &ACTIONS_ZEN } else { &ACTIONS_ALL }
}

// Grid navigation for the species picker: ←→ wrap linearly, ↑↓ move by row.
fn grid_step(idx: usize, len: usize, cols: usize, code: KeyCode) -> usize {
    match code {
        KeyCode::Left | KeyCode::Char('h') => (idx + len - 1) % len,
        KeyCode::Right | KeyCode::Char('l') => (idx + 1) % len,
        KeyCode::Up | KeyCode::Char('k') if idx >= cols => idx - cols,
        KeyCode::Down | KeyCode::Char('j') if idx + cols < len => idx + cols,
        _ => idx,
    }
}

fn push_line(log: &mut VecDeque<Line>, line: Line) {
    log.push_back(line);
    while log.len() > LOG_CAP {
        log.pop_front();
    }
}

fn push_log(log: &mut VecDeque<Line>, time: &str, text: String, suffix: Option<(String, Color)>) {
    let mut l: Line = vec![seg(format!("{time} "), Some(Color::DarkGrey)), seg(text, None)];
    if let Some((s, c)) = suffix {
        l.push(seg(format!(" {s}"), Some(c)));
    }
    push_line(log, l);
}

#[derive(Debug, PartialEq)]
pub enum GameOutcome {
    Draw,
    Win,
    Loss,
}

// Rock-paper-scissors: 0 rock, 1 paper, 2 scissors; each pick beats the previous.
pub fn jokenpo(player: usize, pet_pick: usize) -> GameOutcome {
    if player == pet_pick {
        GameOutcome::Draw
    } else if player == (pet_pick + 1) % 3 {
        GameOutcome::Win
    } else {
        GameOutcome::Loss
    }
}

fn random_pick() -> usize {
    // ponytail: subsecond nanos as rng — one dice roll doesn't justify a rand dependency
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as usize % 3
}

fn mmss(secs: u64) -> String {
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

// Removes and returns the reminders that are due.
fn due_reminders(reminders: &mut Vec<(String, u64)>, now: u64) -> Vec<String> {
    let fired = reminders.iter().filter(|(_, at)| *at <= now).map(|(t, _)| t.clone()).collect();
    reminders.retain(|(_, at)| *at > now);
    fired
}

// A running pomodoro: alternates focus/break phases until stopped.
struct Pomo {
    work: u64,
    rest: u64,
    focus: bool,
    until: u64,
    cycles: u32, // 1-based: which focus block we are on
}

impl From<state::PomoState> for Pomo {
    fn from(p: state::PomoState) -> Self {
        Pomo { work: p.work, rest: p.rest, focus: p.focus, until: p.until, cycles: p.cycles }
    }
}

impl Pomo {
    fn saved(&self) -> state::PomoState {
        state::PomoState {
            work: self.work,
            rest: self.rest,
            focus: self.focus,
            until: self.until,
            cycles: self.cycles,
        }
    }
}

// Flips the phase when the current one ended; returns the phase just entered
// (true = focus).
fn pomo_tick(p: &mut Pomo, now: u64) -> Option<bool> {
    if p.until > now {
        return None;
    }
    p.focus = !p.focus;
    p.until = now + if p.focus { p.work } else { p.rest };
    if p.focus {
        p.cycles += 1;
    }
    Some(p.focus)
}

// Updates the per-source progress list in place, keeping display order stable;
// true when the task hit 100% and was removed.
fn update_progress(progress: &mut Vec<(String, u8)>, from: &str, pct: u8) -> bool {
    if pct >= 100 {
        progress.retain(|(f, _)| f != from);
        return true;
    }
    match progress.iter_mut().find(|(f, _)| f == from) {
        Some(entry) => entry.1 = pct,
        None => progress.push((from.to_string(), pct)),
    }
    false
}

fn queue_preview(m: &Msg) -> Option<String> {
    match m {
        Msg::Say { text, from, .. } | Msg::Ask { text, from, .. } => {
            Some(if from.is_empty() { text.clone() } else { format!("{from}: {text}") })
        }
        _ => None,
    }
}

// Everything the assistant flow needs in the main loop.
struct Inbox {
    queue: VecDeque<Msg>,
    current: Option<(Msg, Instant)>,
}

impl Inbox {
    fn new() -> Self {
        Inbox { queue: VecDeque::new(), current: None }
    }

    fn promote(&mut self) {
        if self.current.is_none() {
            self.current = self.queue.pop_front().map(|m| (m, Instant::now()));
        }
    }

    // Drops the current message; answers a discarded Ask so callers never hang.
    fn advance(&mut self) {
        if let Some((Msg::Ask { id, .. }, _)) = self.current.take() {
            let _ = assistant::write_answer(&id, assistant::ANSWER_IGNORED);
        }
    }

    fn clear(&mut self) {
        self.advance();
        for m in self.queue.drain(..) {
            if let Msg::Ask { id, .. } = m {
                let _ = assistant::write_answer(&id, assistant::ANSWER_IGNORED);
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.current.is_none() && self.queue.is_empty()
    }

    // Drops expired asks WITHOUT writing an answer — the CLI side already gave
    // up and printed its default; returns the senders for the log.
    fn purge_expired(&mut self, now: u64) -> Vec<String> {
        let expired = |m: &Msg| matches!(m, Msg::Ask { expires: Some(e), .. } if *e <= now);
        let mut froms = Vec::new();
        if self.current.as_ref().is_some_and(|(m, _)| expired(m)) {
            if let Some((Msg::Ask { from, .. }, _)) = self.current.take() {
                froms.push(from);
            }
        }
        self.queue.retain(|m| {
            if expired(m) {
                if let Msg::Ask { from, .. } = m {
                    froms.push(from.clone());
                }
                return false;
            }
            true
        });
        froms
    }
}

// A question blocks its sender and you may be in another window — or another
// room, with the tablet on the desk. The bell rings on arrival; on Termux the
// pending question also becomes an Android notification (one, replaced in
// place) that goes away once nothing is pending.
fn notify_cmd(pending: Option<&str>) -> std::process::Command {
    match pending {
        Some(text) => {
            let mut c = std::process::Command::new("termux-notification");
            c.args(["--id", "tama-ask", "--title", "tama", "--content", text]);
            c
        }
        None => {
            let mut c = std::process::Command::new("termux-notification-remove");
            c.arg("tama-ask");
            c
        }
    }
}

fn notify_android(pending: Option<&str>) {
    let mut cmd = notify_cmd(pending);
    // detached: the notification command must not stall the render loop, and
    // waiting in the thread reaps it instead of leaving a zombie behind
    std::thread::spawn(move || {
        let _ = cmd.status();
    });
}

// Quitting with a question on screen must not leave the notification behind;
// here the wait is fine (and necessary — the process is about to end).
fn notify_clear(notified: bool) {
    if notified {
        let _ = notify_cmd(None).status();
    }
}

fn pending_ask(inbox: &Inbox) -> Option<String> {
    let ask = |m: &Msg| match m {
        Msg::Ask { text, from, .. } => {
            Some(if from.is_empty() { text.clone() } else { format!("{from}: {text}") })
        }
        _ => None,
    };
    inbox
        .current
        .as_ref()
        .and_then(|(m, _)| ask(m))
        .or_else(|| inbox.queue.iter().find_map(ask))
}

// The choice list of the question on screen: the sender's options plus the
// "escrever" entry when it accepts free text.
fn ask_options(inbox: &Inbox) -> Option<Vec<String>> {
    match &inbox.current {
        Some((Msg::Ask { options, input, .. }, _)) => Some(ui::option_labels(options, *input)),
        _ => None,
    }
}

// Answers the current question — by picked option or typed text, same path.
fn answer_ask(inbox: &mut Inbox, log: &mut VecDeque<Line>, time: &str, answer: &str) {
    let Some((Msg::Ask { text, id, .. }, _)) = &inbox.current else { return };
    let entry = i18n::msg_answered(text, answer);
    match assistant::write_answer(id, answer) {
        Ok(()) => push_log(log, time, entry, None),
        Err(e) => push_line(log, tinted(format!("{entry} ({e})"), Color::Red)),
    }
    inbox.current = None;
}

fn do_play(pet: &mut Pet, log: &mut VecDeque<Line>, time: &str) -> io::Result<()> {
    let leveled = pet.play();
    push_log(log, time, i18n::msg_played(&pet.name), Some(("(+10 xp)".into(), Color::Cyan)));
    if leveled {
        push_log(log, time, i18n::msg_level_up(&pet.name, pet.level), None);
    }
    save(pet)
}

fn do_bath(pet: &mut Pet, log: &mut VecDeque<Line>, time: &str) -> io::Result<()> {
    let leveled = pet.bathe();
    push_log(log, time, i18n::msg_bathed(&pet.name), Some((i18n::BATH_SUFFIX.into(), Color::Green)));
    if leveled {
        push_log(log, time, i18n::msg_level_up(&pet.name, pet.level), None);
    }
    save(pet)
}

fn do_sleep(pet: &mut Pet, log: &mut VecDeque<Line>, time: &str) {
    pet.sleeping = !pet.sleeping;
    push_log(log, time, i18n::msg_sleep(&pet.name, pet.sleeping), None);
}

fn do_zen(pet: &mut Pet, log: &mut VecDeque<Line>, time: &str) -> io::Result<()> {
    pet.zen = !pet.zen;
    pet.sleeping = false;
    push_log(log, time, i18n::msg_zen(pet.zen), None);
    save(pet)
}

fn do_switch(out: &mut impl Write, pet: &mut Pet, log: &mut VecDeque<Line>, time: &str) -> io::Result<()> {
    let new = pick_species(out, pet.species)?;
    if new != pet.species {
        pet.species = new;
        push_log(log, time, i18n::msg_became(&pet.name, new), None);
        save(pet)?;
    }
    Ok(())
}

// Grid picker from the controls redesign: every species visible at once,
// with an animated preview of the highlighted one below.
fn pick_species(out: &mut impl Write, start: Species) -> io::Result<Species> {
    let mut idx = SPECIES.iter().position(|&s| s == start).unwrap_or(0);
    let mut frame = 0usize;
    const CELL: usize = 15;
    loop {
        let (tcols, trows) = crossterm::terminal::size()?;
        let (iw, ih) = (tcols.saturating_sub(2) as usize, trows.saturating_sub(2) as usize);
        let cols = (iw / CELL).clamp(1, 5).min(SPECIES.len());
        let species = SPECIES[idx];

        let mut content: Vec<Line> = vec![
            vec![
                seg(i18n::PICKER_TITLE, Some(Color::Magenta)),
                seg(format!("  {} ({}/{})", i18n::species_name(species), idx + 1, SPECIES.len()), Some(Color::DarkGrey)),
            ],
            Vec::new(),
        ];
        for row_start in (0..SPECIES.len()).step_by(cols) {
            let mut faces: Line = Vec::new();
            let mut names: Line = Vec::new();
            for (offset, &sp) in SPECIES[row_start..(row_start + cols).min(SPECIES.len())].iter().enumerate() {
                let selected = row_start + offset == idx;
                faces.push(seg(
                    format!("{:^CELL$}", render_tiny(sp, Mood::Happy, if selected { frame } else { 0 })),
                    Some(if selected { Color::Cyan } else { Color::Green }),
                ));
                names.push(seg(
                    format!("{:^CELL$}", i18n::species_name(sp)),
                    Some(if selected { Color::Cyan } else { Color::DarkGrey }),
                ));
            }
            content.push(faces);
            content.push(names);
            content.push(Vec::new());
        }
        let preview = render_art(species, Mood::Happy, frame, ArtSize::Small);
        if ih >= content.len() + preview.len() + 1 && iw >= preview[0].chars().count() {
            content.extend(preview.iter().map(|l| plain(l.clone())));
        }
        draw_screen(out, &content, &i18n::FOOTER_PICKER)?;

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Enter | KeyCode::Char(' ') => return Ok(SPECIES[idx]),
                        KeyCode::Esc => return Ok(start),
                        code => idx = grid_step(idx, SPECIES.len(), cols, code),
                    }
                }
            }
        } else {
            frame += 1;
        }
    }
}

fn ask_name(out: &mut impl Write, species: Species) -> io::Result<String> {
    let mut name = String::new();
    let mut frame = 0usize;
    loop {
        let mut content: Vec<Line> = vec![tinted(i18n::NAME_PROMPT, Color::Magenta), Vec::new()];
        content.extend(render_art(species, Mood::Happy, frame, ArtSize::Small).iter().map(|l| plain(l.clone())));
        content.push(Vec::new());
        content.push(tinted(format!("> {name}_"), Color::Cyan));
        draw_screen(out, &content, &i18n::FOOTER_NAME)?;

        if event::poll(Duration::from_millis(500))? {
            if let Event::Key(k) = event::read()? {
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Enter => {
                            let name = name.trim().to_string();
                            return Ok(if name.is_empty() { i18n::DEFAULT_NAME.to_string() } else { name });
                        }
                        KeyCode::Backspace => {
                            name.pop();
                        }
                        // restricted to keep the key=value state file unambiguous
                        KeyCode::Char(c)
                            if (c.is_alphanumeric() || c == ' ' || c == '-') && name.chars().count() < 12 =>
                        {
                            name.push(c);
                        }
                        _ => {}
                    }
                }
            }
        } else {
            frame += 1;
        }
    }
}

pub fn run(out: &mut impl Write, pet: &mut Pet, is_new: bool) -> io::Result<()> {
    let mut clock = Clock::new();
    if is_new {
        pet.species = pick_species(out, pet.species)?;
        save(pet)?;
    }
    if pet.name.is_empty() {
        pet.name = ask_name(out, pet.species)?;
        save(pet)?;
    }

    // FIFO and HTTP feed the same channel; the loop drains one stream.
    let (tx, rx) = std::sync::mpsc::channel();
    assistant::spawn_reader(tx.clone());
    let http_status = crate::http::spawn_http(tx);
    // Stale answers would satisfy the wrong ask; a fresh session starts clean.
    // ponytail: races a CLI still polling from a previous session; accepted
    let _ = std::fs::write(crate::state::output_path(), "");
    let mut inbox = Inbox::new();
    let mut progress: Vec<(String, u8)> = Vec::new();
    // reminders, timer and pomodoro outlive the process: Android kills Termux
    // whenever it feels like it, and "me lembra em 10min" must survive that
    let saved = state::load_schedule(assistant::now_epoch());
    let mut reminders: Vec<(String, u64)> = saved.reminders;
    let mut timer_until: Option<u64> = saved.timer;
    let mut pomo: Option<Pomo> = saved.pomo.map(Pomo::from);
    let mut schedule_dirty = false;
    let mut notified = false; // an Android notification is currently showing
    let mut ask_sel = 0usize; // highlighted option of the question on screen
    let mut ask_id = String::new(); // which question that state belongs to

    let mut screen = Screen::Home;
    // where an Ask yanked the user from; restored when the inbox drains
    let mut prev_screen = Screen::Home;
    // typed-answer buffer: Some = the current ask is being answered by text
    let mut input: Option<String> = None;
    let mut frame = 0usize;
    let mut ticks_250ms = 0u64;
    let mut prev_mood = pet.mood();
    let mut log: VecDeque<Line> = VecDeque::new();
    push_line(&mut log, tinted(http_status, Color::DarkGrey));

    loop {
        let (time, hour) = clock.get();
        let now = assistant::now_epoch();

        // Drain external messages (pipe + HTTP share the channel).
        for line in rx.try_iter().collect::<Vec<_>>() {
            for msg in assistant::parse_msgs(&line, now) {
            match msg {
                Msg::Say { ref text, ref from, kind } => {
                    let sender = if from.is_empty() { i18n::UNKNOWN_SENDER } else { from };
                    push_line(&mut log, vec![
                        seg(format!("{time} "), Some(Color::DarkGrey)),
                        seg(format!("{sender}: "), Some(Color::DarkGrey)),
                        seg(text.clone(), Some(ui::kind_color(kind))),
                    ]);
                    inbox.queue.push_back(msg);
                    if matches!(screen, Screen::Home) {
                        screen = Screen::Assistant;
                    }
                }
                Msg::Ask { .. } => {
                    inbox.queue.push_front(msg); // questions jump the queue
                    let _ = out.write_all(b"\x07"); // flushed with the next frame

                    // a question blocks its sender: interrupt from ANY screen
                    if !matches!(screen, Screen::Assistant) {
                        prev_screen = screen;
                        screen = Screen::Assistant;
                    }
                }
                Msg::Action(a) => {
                    match a.as_str() {
                        "celebrate" => {
                            pet.happiness = adj(pet.happiness, 15);
                            pet.gain_xp(10);
                            inbox.queue.push_back(Msg::Say {
                                text: i18n::msg_celebrate(&pet.name),
                                from: String::new(),
                                kind: Kind::Success,
                            });
                            if matches!(screen, Screen::Home) {
                                screen = Screen::Assistant;
                            }
                            save(pet)?;
                        }
                        "sleep" => {
                            pet.sleeping = true;
                            push_log(&mut log, &time, i18n::msg_sleep(&pet.name, true), None);
                        }
                        "wake" => {
                            pet.sleeping = false;
                            push_log(&mut log, &time, i18n::msg_sleep(&pet.name, false), None);
                        }
                        "feed" => {
                            pet.eat(&FOODS[0]);
                            push_log(&mut log, &time, i18n::msg_action_fed(&pet.name), Some(("(+15 fome)".into(), Color::Green)));
                            save(pet)?;
                        }
                        _ => {}
                    }
                }
                Msg::Progress { from, pct } => {
                    if update_progress(&mut progress, &from, pct) {
                        push_log(&mut log, &time, i18n::msg_progress_done(&from), Some(("(100%)".into(), Color::Green)));
                    }
                }
                Msg::Reminder { text, at } => {
                    reminders.push((text, at));
                    schedule_dirty = true;
                }
                Msg::Timer { until } => {
                    timer_until = Some(until);
                    schedule_dirty = true;
                }
                Msg::Pomodoro { work, rest } => {
                    pomo = Some(Pomo { work, rest, focus: true, until: now + work, cycles: 1 });
                    schedule_dirty = true;
                    pet.sleeping = false;
                    push_log(&mut log, &time, i18n::MSG_POMO_START.into(), None);
                    // a starting pomodoro opens its screen, like messages do
                    if matches!(screen, Screen::Home) {
                        screen = Screen::Pomo(0);
                    }
                }
                Msg::PomodoroOff => {
                    if pomo.take().is_some() {
                        schedule_dirty = true;
                        pet.sleeping = false;
                        push_log(&mut log, &time, i18n::MSG_POMO_STOPPED.into(), None);
                    }
                }
            }
            }
        }

        // Fire due reminders and the timer.
        let fired = due_reminders(&mut reminders, now);
        schedule_dirty |= !fired.is_empty();
        for text in fired {
            inbox.queue.push_back(Msg::Say { text: i18n::msg_reminder(&text), from: String::new(), kind: Kind::Warn });
            if matches!(screen, Screen::Home) {
                screen = Screen::Assistant;
            }
        }
        if timer_until.is_some_and(|u| u <= now) {
            timer_until = None;
            schedule_dirty = true;
            inbox.queue.push_back(Msg::Say { text: i18n::MSG_TIMER_DONE.into(), from: String::new(), kind: Kind::Warn });
            if matches!(screen, Screen::Home) {
                screen = Screen::Assistant;
            }
        }
        if let Some(p) = &mut pomo {
            if let Some(focus) = pomo_tick(p, now) {
                schedule_dirty = true;
                pet.sleeping = !focus; // the pet rests with you on breaks
                let text = if focus { i18n::MSG_POMO_FOCUS } else { i18n::MSG_POMO_BREAK };
                if matches!(screen, Screen::Pomo(_)) {
                    // the screen itself shows the change (title, color, pet);
                    // just keep the record in the log
                    push_log(&mut log, &time, text.into(), None);
                } else {
                    inbox.queue.push_back(Msg::Say {
                        text: text.into(),
                        from: i18n::POMO_FROM.into(),
                        kind: if focus { Kind::Success } else { Kind::Warn },
                    });
                    if matches!(screen, Screen::Home) {
                        screen = Screen::Assistant;
                    }
                }
            }
        }

        // A pomodoro break IS nap time: overrides the full-energy auto-wake
        // so the pet keeps napping until the break actually ends.
        if pomo.as_ref().is_some_and(|p| !p.focus) {
            pet.sleeping = true;
        }

        // Expired asks vanish everywhere, even while another screen is up.
        for from in inbox.purge_expired(now) {
            let sender = if from.is_empty() { i18n::UNKNOWN_SENDER.to_string() } else { from };
            push_log(&mut log, &time, i18n::msg_ask_expired(&sender), None);
        }

        // A spoken message expires on its own; questions wait for an answer.
        if matches!(screen, Screen::Assistant) {
            if let Some((Msg::Say { .. }, at)) = &inbox.current {
                if at.elapsed() >= Duration::from_secs(SAY_SECS) {
                    inbox.current = None;
                }
            }
            inbox.promote();
            // cursor and typing buffer belong to ONE question: a new one on
            // screen starts clean, and a text-only ask opens the field itself
            match &inbox.current {
                Some((Msg::Ask { id, options, .. }, _)) if *id != ask_id => {
                    ask_id = id.clone();
                    ask_sel = 0;
                    input = options.is_empty().then(String::new);
                }
                Some((Msg::Ask { .. }, _)) => {}
                _ => {
                    ask_id.clear();
                    input = None;
                }
            }
            if inbox.is_empty() {
                screen = prev_screen;
                prev_screen = Screen::Home;
            }
        }

        if schedule_dirty {
            let _ = state::save_schedule(&state::Schedule {
                reminders: reminders.clone(),
                timer: timer_until,
                pomo: pomo.as_ref().map(Pomo::saved),
            });
            schedule_dirty = false;
        }

        // One notification tracks "there is a question waiting", whatever
        // ended it: answered, ignored, expired or the app closing.
        let pending = pending_ask(&inbox);
        if pending.is_some() != notified {
            notify_android(pending.as_deref());
            notified = pending.is_some();
        }

        let view = HomeView {
            log: &log,
            clock_text: &time,
            hour,
            // an active pomodoro owns the header slot; a plain timer otherwise
            timer: pomo
                .as_ref()
                .map(|p| (if p.focus { i18n::POMO_FOCUS } else { i18n::POMO_BREAK }, mmss(p.until.saturating_sub(now))))
                .or_else(|| timer_until.map(|u| (i18n::TIMER_LABEL, mmss(u.saturating_sub(now))))),
            progress: progress.iter().map(|(from, pct)| ui::progress_line(from, *pct)).collect(),
        };
        match &screen {
            Screen::Home => ui::draw_home(out, pet, frame, &view)?,
            Screen::Actions(sel) => {
                let items: Vec<usize> = actions_for(pet.zen).iter().map(|a| *a as usize).collect();
                ui::draw_actions(out, pet, frame, &view, &items, *sel)?;
            }
            Screen::Menu(sel) => ui::draw_menu(out, pet, frame, &view, *sel)?,
            Screen::Game => ui::draw_game(out, pet, frame, &view)?,
            Screen::Pomo(sel) => {
                let run = pomo.as_ref().map(|p| {
                    let duration = if p.focus { p.work } else { p.rest }.max(1);
                    let remaining = p.until.saturating_sub(now).min(duration);
                    ui::PomoRun {
                        label: if p.focus { i18n::POMO_FOCUS } else { i18n::POMO_BREAK },
                        focus: p.focus,
                        frac: (100 - remaining * 100 / duration) as u8,
                        cycle: p.cycles,
                    }
                });
                // idle: the clock previews the selected preset's focus length
                let clock = match &pomo {
                    Some(p) => mmss(p.until.saturating_sub(now)),
                    None => mmss(POMO_PRESETS[*sel].0),
                };
                ui::draw_pomo(out, pet, frame, &view, &clock, run.as_ref(), *sel)?;
            }
            Screen::Assistant => {
                let current = inbox.current.as_ref().map(|(m, _)| m);
                let msg = current.and_then(|m| match m {
                    Msg::Say { text, from, kind } => Some(ui::AssistantMsg {
                        text,
                        from,
                        kind: *kind,
                        kind_label: i18n::kind_label(*kind),
                        options: None,
                        expires_in: None,
                        input: None,
                        input_ok: false,
                        sel: 0,
                    }),
                    Msg::Ask { text, from, options, expires, input: input_ok, .. } => Some(ui::AssistantMsg {
                        text,
                        from,
                        kind: Kind::Info,
                        kind_label: i18n::kind_label(Kind::Info),
                        options: Some(options),
                        expires_in: expires.map(|e| e.saturating_sub(now)),
                        input: input.as_deref(),
                        input_ok: *input_ok,
                        sel: ask_sel,
                    }),
                    _ => None,
                });
                let preview: Vec<String> = inbox.queue.iter().filter_map(queue_preview).collect();
                ui::draw_assistant(out, pet, frame, msg.as_ref(), &preview, inbox.queue.len(), &view)?;
            }
        }

        if event::poll(Duration::from_millis(250))? {
            let Event::Key(k) = event::read()? else { continue };
            if k.kind != KeyEventKind::Press {
                continue;
            }
            match screen {
                Screen::Home => match k.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        inbox.clear();
                        notify_clear(notified);
                        return Ok(());
                    }
                    KeyCode::Char(' ') => screen = Screen::Actions(0),
                    KeyCode::Char('a') => screen = Screen::Assistant,
                    // legacy shortcuts, hidden from the footer but kept working
                    KeyCode::Char('f') if !pet.zen => screen = Screen::Menu(0),
                    KeyCode::Char('m') if !pet.zen => screen = Screen::Game,
                    KeyCode::Char('p') if !pet.zen => do_play(pet, &mut log, &time)?,
                    KeyCode::Char('s') if !pet.zen => do_sleep(pet, &mut log, &time),
                    KeyCode::Char('b') if !pet.zen => do_bath(pet, &mut log, &time)?,
                    KeyCode::Char('z') => do_zen(pet, &mut log, &time)?,
                    KeyCode::Char('c') => do_switch(out, pet, &mut log, &time)?,
                    _ => {}
                },
                Screen::Actions(sel) => {
                    let items = actions_for(pet.zen);
                    let chosen = match k.code {
                        KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                            screen = Screen::Home;
                            None
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            screen = Screen::Actions((sel + items.len() - 1) % items.len());
                            None
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            screen = Screen::Actions((sel + 1) % items.len());
                            None
                        }
                        KeyCode::Enter => items.get(sel).copied(),
                        KeyCode::Char(c @ '1'..='9') => items.get(c as usize - '1' as usize).copied(),
                        _ => None,
                    };
                    if let Some(action) = chosen {
                        screen = Screen::Home;
                        match action {
                            Action::Feed => screen = Screen::Menu(0),
                            Action::Game => screen = Screen::Game,
                            Action::Assistant => screen = Screen::Assistant,
                            Action::Pomo => screen = Screen::Pomo(0),
                            Action::Play => do_play(pet, &mut log, &time)?,
                            Action::Sleep => do_sleep(pet, &mut log, &time),
                            Action::Bath => do_bath(pet, &mut log, &time)?,
                            Action::Zen => do_zen(pet, &mut log, &time)?,
                            Action::Switch => do_switch(out, pet, &mut log, &time)?,
                        }
                    }
                }
                Screen::Menu(sel) => match k.code {
                    KeyCode::Esc | KeyCode::Char('q') => screen = Screen::Home,
                    KeyCode::Up | KeyCode::Char('k') => screen = Screen::Menu((sel + FOODS.len() - 1) % FOODS.len()),
                    KeyCode::Down | KeyCode::Char('j') => screen = Screen::Menu((sel + 1) % FOODS.len()),
                    KeyCode::Enter => {
                        let food = &FOODS[sel];
                        let leveled = pet.eat(food);
                        let suffix = if food.hunger > 0 {
                            format!("(+{} {})", food.hunger, i18n::STAT_LABELS[0])
                        } else {
                            format!("(+{} {})", food.energy, i18n::STAT_LABELS[2])
                        };
                        push_log(&mut log, &time, i18n::msg_fed(i18n::FOOD_NAMES[sel], &pet.name), Some((suffix, Color::Green)));
                        if leveled {
                            push_log(&mut log, &time, i18n::msg_level_up(&pet.name, pet.level), None);
                        }
                        save(pet)?;
                        screen = Screen::Home;
                    }
                    _ => {}
                },
                Screen::Game => match k.code {
                    KeyCode::Esc | KeyCode::Char('q') => screen = Screen::Home,
                    KeyCode::Char(c @ '1'..='3') => {
                        let player = c as usize - '1' as usize;
                        let pet_pick = random_pick();
                        // the pet winning makes the PET happier — by design
                        let (label, happy, xp) = match jokenpo(player, pet_pick) {
                            GameOutcome::Draw => (i18n::GAME_DRAW, 5, 5),
                            GameOutcome::Win => (i18n::GAME_WIN, 5, 10),
                            GameOutcome::Loss => (i18n::GAME_LOSS, 15, 20),
                        };
                        pet.happiness = adj(pet.happiness, happy);
                        pet.energy = adj(pet.energy, -5);
                        let leveled = pet.gain_xp(xp);
                        push_log(
                            &mut log,
                            &time,
                            i18n::msg_game(i18n::HANDS[player], i18n::HANDS[pet_pick], label),
                            Some((format!("(+{xp} xp)"), Color::Cyan)),
                        );
                        if leveled {
                            push_log(&mut log, &time, i18n::msg_level_up(&pet.name, pet.level), None);
                        }
                        save(pet)?;
                        screen = Screen::Home;
                    }
                    _ => {}
                },
                Screen::Pomo(sel) => {
                    let start = match k.code {
                        KeyCode::Esc | KeyCode::Char('q') => {
                            screen = Screen::Home;
                            None
                        }
                        KeyCode::Up | KeyCode::Char('k') if pomo.is_none() => {
                            screen = Screen::Pomo((sel + POMO_PRESETS.len() - 1) % POMO_PRESETS.len());
                            None
                        }
                        KeyCode::Down | KeyCode::Char('j') if pomo.is_none() => {
                            screen = Screen::Pomo((sel + 1) % POMO_PRESETS.len());
                            None
                        }
                        // enter starts the selected preset — or stops the running cycle
                        KeyCode::Enter => match pomo.take() {
                            Some(_) => {
                                schedule_dirty = true;
                                pet.sleeping = false;
                                push_log(&mut log, &time, i18n::MSG_POMO_STOPPED.into(), None);
                                None
                            }
                            None => POMO_PRESETS.get(sel).copied(),
                        },
                        KeyCode::Char(c @ '1'..='9') if pomo.is_none() => {
                            POMO_PRESETS.get(c as usize - '1' as usize).copied()
                        }
                        _ => None,
                    };
                    if let Some((work, rest)) = start {
                        pomo = Some(Pomo { work, rest, focus: true, until: now + work, cycles: 1 });
                        schedule_dirty = true;
                        pet.sleeping = false;
                        push_log(&mut log, &time, i18n::MSG_POMO_START.into(), None);
                        // stay here: this screen IS the focus mode
                    }
                }
                // Typing a free-text answer: every key belongs to the field
                // (that is why it is matched before the shortcut table).
                Screen::Assistant if input.is_some() => {
                    let buf = input.as_mut().unwrap();
                    match k.code {
                        // alt+enter breaks the line; plain enter sends, which
                        // is the reachable one on a phone keyboard
                        KeyCode::Enter
                            if k.modifiers.contains(event::KeyModifiers::ALT)
                                && buf.chars().count() < ANSWER_CAP =>
                        {
                            buf.push('\n');
                        }
                        KeyCode::Enter if !buf.trim().is_empty() => {
                            let answer = buf.trim().to_string();
                            answer_ask(&mut inbox, &mut log, &time, &answer);
                            input = None;
                        }
                        KeyCode::Backspace => {
                            buf.pop();
                        }
                        // esc leaves the field; on a text-only ask there is
                        // nothing to go back to, so it discards the question
                        KeyCode::Esc => {
                            let text_only = matches!(&inbox.current, Some((Msg::Ask { options, .. }, _)) if options.is_empty());
                            input = None;
                            if text_only {
                                inbox.advance();
                            }
                        }
                        KeyCode::Char(c) if buf.chars().count() < ANSWER_CAP => buf.push(c),
                        _ => {}
                    }
                }
                Screen::Assistant => match k.code {
                    KeyCode::Char('q') => {
                        inbox.clear();
                        notify_clear(notified);
                        return Ok(());
                    }
                    KeyCode::Char('a') => {
                        screen = prev_screen;
                        prev_screen = Screen::Home;
                    }
                    KeyCode::Char('x') => {
                        inbox.clear();
                        screen = prev_screen;
                        prev_screen = Screen::Home;
                    }
                    // shortcut for the same "escrever" entry listed below
                    KeyCode::Char('t') if matches!(inbox.current, Some((Msg::Ask { input: true, .. }, _))) => {
                        input = Some(String::new());
                    }
                    // cursor over the option list, like the actions menu; the
                    // list scrolls when it is longer than the visible slots
                    KeyCode::Up | KeyCode::Down | KeyCode::Char('k') | KeyCode::Char('j') => {
                        if let Some(len) = ask_options(&inbox).map(|o| o.len()) {
                            let back = matches!(k.code, KeyCode::Up | KeyCode::Char('k'));
                            ask_sel = match back {
                                true => (ask_sel + len - 1) % len,
                                false => (ask_sel + 1) % len,
                            };
                        }
                    }
                    KeyCode::Enter => match ask_options(&inbox).and_then(|o| o.get(ask_sel).cloned()) {
                        Some(o) if o == i18n::OPTION_WRITE => input = Some(String::new()),
                        Some(option) => answer_ask(&mut inbox, &mut log, &time, &option),
                        // no question on screen: enter dismisses a message
                        None => {
                            if matches!(inbox.current, Some((Msg::Say { .. }, _))) {
                                inbox.current = None;
                            }
                        }
                    },
                    KeyCode::Esc => {
                        inbox.advance();
                    }
                    // The numbered list is options + (when free text is
                    // accepted) one last "escrever" entry, which opens the
                    // field instead of answering.
                    KeyCode::Char(c @ '1'..='9') => {
                        let picked =
                            ask_options(&inbox).and_then(|o| o.get(c as usize - '1' as usize).cloned());
                        match picked {
                            Some(o) if o == i18n::OPTION_WRITE => input = Some(String::new()),
                            Some(option) => answer_ask(&mut inbox, &mut log, &time, &option),
                            None => {}
                        }
                    }
                    _ => {}
                },
            }
        } else {
            ticks_250ms += 1;
            if ticks_250ms % 2 == 0 {
                frame += 1; // animation clock: ~500ms
            }
            // ponytail: sleeping recovers on a fast clock (2s), normal decay on DECAY_SECS
            if pet.sleeping && ticks_250ms % 8 == 0 {
                pet.tick();
            } else if ticks_250ms % (DECAY_SECS * 4) == 0 {
                pet.tick();
            }
            let mood = pet.mood();
            if mood != prev_mood {
                if let Some(w) = i18n::msg_warning(mood, &pet.name) {
                    push_line(&mut log, vec![
                        seg(format!("{time} "), Some(Color::DarkGrey)),
                        seg(w, Some(ui::mood_color(mood))),
                    ]);
                }
                prev_mood = mood;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jokenpo_covers_all_nine_combinations() {
        let mut draws = 0;
        let mut wins = 0;
        let mut losses = 0;
        for player in 0..3 {
            for pet in 0..3 {
                match jokenpo(player, pet) {
                    GameOutcome::Draw => draws += 1,
                    GameOutcome::Win => wins += 1,
                    GameOutcome::Loss => losses += 1,
                }
            }
        }
        assert_eq!((draws, wins, losses), (3, 3, 3));
    }

    #[test]
    fn jokenpo_paper_beats_rock() {
        assert_eq!(jokenpo(1, 0), GameOutcome::Win);
        assert_eq!(jokenpo(0, 1), GameOutcome::Loss);
        assert_eq!(jokenpo(2, 2), GameOutcome::Draw);
    }

    #[test]
    fn random_pick_is_a_valid_hand() {
        for _ in 0..10 {
            assert!(random_pick() < 3);
        }
    }

    #[test]
    fn grid_step_navigates_two_axes_with_wrap() {
        use KeyCode::*;
        let (len, cols) = (10, 5);
        assert_eq!(grid_step(0, len, cols, Right), 1);
        assert_eq!(grid_step(0, len, cols, Left), 9);
        assert_eq!(grid_step(9, len, cols, Right), 0);
        assert_eq!(grid_step(2, len, cols, Down), 7);
        assert_eq!(grid_step(7, len, cols, Up), 2);
        assert_eq!(grid_step(2, len, cols, Up), 2); // top row stays
        assert_eq!(grid_step(7, len, cols, Down), 7); // bottom row stays
    }

    #[test]
    fn action_tables_stay_index_aligned() {
        assert_eq!(ACTIONS_ALL.len(), crate::i18n::ACTION_LABELS.len());
        assert_eq!(ACTIONS_ALL.len(), crate::ui::ACTION_GLYPHS.len());
        for (i, a) in ACTIONS_ALL.iter().enumerate() {
            assert_eq!(*a as usize, i);
        }
        for a in ACTIONS_ZEN {
            assert!(ACTIONS_ALL.contains(&a));
        }
        assert_eq!(POMO_PRESETS.len(), crate::i18n::POMO_PRESET_LABELS.len());
    }

    #[test]
    fn mmss_formats_minutes_and_hours() {
        assert_eq!(mmss(0), "00:00");
        assert_eq!(mmss(1062), "17:42");
        assert_eq!(mmss(3661), "1:01:01");
    }

    #[test]
    fn pomo_tick_alternates_phases_with_their_own_durations() {
        let mut p = Pomo { work: 1500, rest: 300, focus: true, until: 100, cycles: 1 };
        assert_eq!(pomo_tick(&mut p, 50), None); // still running
        assert_eq!(pomo_tick(&mut p, 100), Some(false)); // break starts
        assert_eq!((p.until, p.cycles), (400, 1));
        assert_eq!(pomo_tick(&mut p, 400), Some(true)); // focus resumes
        assert_eq!((p.until, p.cycles), (1900, 2));
    }

    #[test]
    fn update_progress_tracks_sources_independently() {
        let mut progress = Vec::new();
        assert!(!update_progress(&mut progress, "build", 10));
        assert!(!update_progress(&mut progress, "deploy", 50));
        assert!(!update_progress(&mut progress, "build", 90)); // updates in place
        assert_eq!(progress, vec![("build".to_string(), 90), ("deploy".to_string(), 50)]);
        assert!(update_progress(&mut progress, "build", 100)); // done → removed
        assert_eq!(progress, vec![("deploy".to_string(), 50)]);
    }

    #[test]
    fn due_reminders_fire_once_and_keep_the_rest() {
        let mut rs = vec![("a".to_string(), 10), ("b".to_string(), 20)];
        assert_eq!(due_reminders(&mut rs, 15), vec!["a".to_string()]);
        assert_eq!(rs.len(), 1);
        assert!(due_reminders(&mut rs, 15).is_empty());
    }

    #[test]
    fn questions_jump_the_queue_and_says_expire() {
        let mut inbox = Inbox::new();
        inbox.queue.push_back(Msg::Say { text: "s".into(), from: String::new(), kind: Kind::Info });
        inbox.queue.push_front(Msg::Ask {
            text: "q".into(),
            options: vec!["sim".into()],
            id: "i".into(),
            from: String::new(),
            expires: None,
            input: false,
        });
        inbox.promote();
        assert!(matches!(inbox.current, Some((Msg::Ask { .. }, _))));
    }

    #[test]
    fn purge_expired_drops_only_expired_asks_and_reports_senders() {
        let ask = |id: &str, from: &str, expires: Option<u64>| Msg::Ask {
            text: "q".into(),
            options: vec!["sim".into()],
            id: id.into(),
            from: from.into(),
            expires,
            input: false,
        };
        let mut inbox = Inbox::new();
        inbox.queue.push_back(ask("a", "claude", Some(100)));
        inbox.queue.push_back(ask("b", "ci", None));
        inbox.queue.push_back(ask("c", "outro", Some(500)));
        inbox.promote(); // "a" becomes current
        assert_eq!(inbox.purge_expired(100), vec!["claude".to_string()]);
        assert!(inbox.current.is_none());
        assert_eq!(inbox.queue.len(), 2); // "b" (sem expira) e "c" (ainda viva) ficam
        assert!(inbox.purge_expired(100).is_empty());
    }

    #[test]
    fn pending_ask_drives_the_notification_and_ignores_plain_messages() {
        let mut inbox = Inbox::new();
        assert_eq!(pending_ask(&inbox), None);
        inbox.queue.push_back(Msg::Say { text: "oi".into(), from: "ci".into(), kind: Kind::Info });
        assert_eq!(pending_ask(&inbox), None, "uma fala não é pergunta pendente");
        inbox.queue.push_back(Msg::Ask {
            text: "posso?".into(),
            options: vec!["sim".into()],
            id: "i".into(),
            from: "claude".into(),
            expires: None,
            input: false,
        });
        // queued or current, a waiting question always shows up
        assert_eq!(pending_ask(&inbox), Some("claude: posso?".to_string()));
        inbox.promote();
        inbox.current = None; // the say was promoted and dismissed
        inbox.promote();
        assert_eq!(pending_ask(&inbox), Some("claude: posso?".to_string()));
        inbox.advance();
        assert_eq!(pending_ask(&inbox), None);
    }

    #[test]
    fn queue_preview_only_covers_visible_messages() {
        assert!(queue_preview(&Msg::Say { text: "x".into(), from: "ci".into(), kind: Kind::Info })
            .is_some_and(|p| p == "ci: x"));
        assert!(queue_preview(&Msg::Action("dormir".into())).is_none());
    }
}
