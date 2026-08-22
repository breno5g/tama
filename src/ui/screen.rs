//! The frame every screen is drawn into: outer border, centered content, and
//! the footer row. Also the terminal teardown, which must run even on panic.

use std::io::{self, Write};

use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, execute, queue, terminal};

use super::line::{chip, line_w, seg};
use super::Line;

// Converts "[f] comer  [p] brincar" into chip segments + grey labels.
fn footer_line(s: &str) -> Line {
    let mut out: Line = Vec::new();
    let mut text = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '[' {
            if !text.is_empty() {
                out.push(seg(std::mem::take(&mut text), Some(Color::DarkGrey)));
            }
            let mut key = String::new();
            for k in chars.by_ref() {
                if k == ']' {
                    break;
                }
                key.push(k);
            }
            out.push(chip(&key));
        } else {
            text.push(c);
        }
    }
    if !text.is_empty() {
        out.push(seg(text, Some(Color::DarkGrey)));
    }
    out
}

fn print_line(out: &mut impl Write, row: u16, iw: usize, line: &Line) -> io::Result<()> {
    let total = line_w(line).min(iw);
    let lpad = (iw - total) / 2;
    let rpad = iw - total - lpad;
    queue!(out, cursor::MoveTo(1, row), Print(" ".repeat(lpad)))?;
    let mut budget = total;
    for (s, fg, bg) in line {
        if budget == 0 {
            break;
        }
        // External text reaches here verbatim: a newline or tab would move
        // the cursor mid-frame and shear the layout. One char in, one out,
        // so the width accounting stays honest.
        let t: String = s.chars().take(budget).map(|c| if c.is_control() { ' ' } else { c }).collect();
        budget -= t.chars().count();
        if let Some(c) = fg {
            queue!(out, SetForegroundColor(*c))?;
        }
        if let Some(c) = bg {
            queue!(out, SetBackgroundColor(*c))?;
        }
        queue!(out, Print(t), ResetColor)?;
    }
    queue!(out, Print(" ".repeat(rpad)))
}

// Draws the border, centers `content` in the inner area and pins the widest
// fitting footer candidate to the bottom inner row. Never clears the screen:
// every cell of every frame is repainted, so the previous frame is overwritten
// in place — no blank state, no flicker, even in terminals/tmux without
// synchronized-update support. One flush per frame.
pub fn draw_screen(out: &mut impl Write, content: &[Line], footers: &[&str]) -> io::Result<()> {
    let (cols, rows) = terminal::size()?;
    queue!(out, terminal::BeginSynchronizedUpdate)?;
    if cols < 4 || rows < 3 {
        queue!(out, terminal::Clear(terminal::ClearType::All), terminal::EndSynchronizedUpdate)?;
        return out.flush();
    }

    let iw = cols as usize - 2;
    let ih = rows as usize - 2;
    let horiz = "─".repeat(iw);
    queue!(out, cursor::MoveTo(0, 0))?;
    queue!(out, SetForegroundColor(Color::DarkGrey), Print(format!("┌{horiz}┐")), ResetColor)?;
    queue!(out, cursor::MoveTo(0, rows - 1))?;
    queue!(out, SetForegroundColor(Color::DarkGrey), Print(format!("└{horiz}┘")), ResetColor)?;

    let footer = footers.iter().find(|f| f.chars().count() <= iw);
    let avail = ih - footer.map_or(0, |_| 1);
    let shown = &content[..content.len().min(avail)];
    let top = avail.saturating_sub(shown.len()) / 2;
    let empty: Line = Vec::new();

    for r in 0..ih {
        let row = (r + 1) as u16;
        queue!(out, cursor::MoveTo(0, row))?;
        queue!(out, SetForegroundColor(Color::DarkGrey), Print("│"), ResetColor)?;
        let line: Line;
        let l = if footer.is_some() && r == ih - 1 {
            line = footer_line(footer.unwrap());
            &line
        } else {
            match r.checked_sub(top).filter(|i| *i < shown.len()) {
                Some(i) => &shown[i],
                None => &empty,
            }
        };
        print_line(out, row, iw, l)?;
        queue!(out, cursor::MoveTo(cols - 1, row))?;
        queue!(out, SetForegroundColor(Color::DarkGrey), Print("│"), ResetColor)?;
    }
    queue!(out, terminal::EndSynchronizedUpdate)?;
    out.flush()
}

pub fn restore_terminal() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(io::stdout(), terminal::LeaveAlternateScreen, cursor::Show);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_chars_never_reach_the_terminal() {
        // a newline mid-frame would move the cursor and shear the layout
        let mut out: Vec<u8> = Vec::new();
        print_line(&mut out, 0, 20, &vec![seg("a\nb\tc", None)]).unwrap();
        let painted = String::from_utf8_lossy(&out);
        assert!(painted.contains("a b c"), "{painted:?}");
        assert!(!painted.contains('\n') && !painted.contains('\t'));
    }
}
