//! Per-frame view assembly and the draw dispatch.

use std::io::{self, Write};

use super::inbox::queue_preview;
use super::pomo::{mmss, POMO_PRESETS};
use super::screen::{actions_for, Screen};
use super::{App, Msg};
use crate::assistant::Kind;
use crate::i18n;
use crate::ui::{self, HomeView};

impl App<'_> {
    fn view(&self) -> HomeView<'_> {
        HomeView {
            log: &self.log,
            clock_text: &self.time,
            hour: self.hour,
            // an active pomodoro owns the header slot; a plain timer otherwise
            timer: self
                .pomo
                .as_ref()
                .map(|p| {
                    let label = if p.focus { i18n::t().pomo_focus } else { i18n::t().pomo_break };
                    (label, mmss(p.until.saturating_sub(self.now)))
                })
                .or_else(|| {
                    self.timer_until.map(|u| (i18n::t().timer_label, mmss(u.saturating_sub(self.now))))
                }),
            progress: self.progress.iter().map(|(from, pct)| ui::progress_line(from, *pct)).collect(),
        }
    }

    pub(super) fn draw(&self, out: &mut impl Write) -> io::Result<()> {
        let pet = &*self.pet;
        let view = self.view();
        match &self.screen {
            Screen::Home => ui::draw_home(out, pet, self.frame, &view),
            Screen::Actions(sel) => {
                let items: Vec<usize> = actions_for(pet.zen).iter().map(|a| *a as usize).collect();
                ui::draw_actions(out, pet, self.frame, &view, &items, *sel)
            }
            Screen::Menu(sel) => ui::draw_menu(out, pet, self.frame, &view, *sel),
            Screen::Game => ui::draw_game(out, pet, self.frame, &view),
            Screen::Pomo(sel) => {
                let run = self.pomo.as_ref().map(|p| {
                    let duration = if p.focus { p.work } else { p.rest }.max(1);
                    let remaining = p.until.saturating_sub(self.now).min(duration);
                    ui::PomoRun {
                        label: if p.focus { i18n::t().pomo_focus } else { i18n::t().pomo_break },
                        focus: p.focus,
                        frac: (100 - remaining * 100 / duration) as u8,
                        cycle: p.cycles,
                    }
                });
                // idle: the clock previews the selected preset's focus length
                let clock = match &self.pomo {
                    Some(p) => mmss(p.until.saturating_sub(self.now)),
                    None => mmss(POMO_PRESETS[*sel].0),
                };
                ui::draw_pomo(out, pet, self.frame, &view, &clock, run.as_ref(), *sel)
            }
            Screen::Assistant => {
                let current = self.inbox.current.as_ref().map(|(m, _)| m);
                let msg = current.and_then(|m| self.assistant_msg(m));
                let preview: Vec<String> = self.inbox.queue.iter().filter_map(queue_preview).collect();
                ui::draw_assistant(
                    out,
                    pet,
                    self.frame,
                    msg.as_ref(),
                    &preview,
                    self.inbox.queue.len(),
                    &view,
                )
            }
        }
    }

    fn assistant_msg<'m>(&'m self, m: &'m Msg) -> Option<ui::AssistantMsg<'m>> {
        match m {
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
                expires_in: expires.map(|e| e.saturating_sub(self.now)),
                input: self.input.as_deref(),
                input_ok: *input_ok,
                sel: self.ask_sel,
            }),
            _ => None,
        }
    }
}
