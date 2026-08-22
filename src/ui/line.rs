//! Line primitives: a `Line` is a list of colored `Seg`ments and everything
//! drawn is composed from these. They all measure WIDTH IN CHARS, never bytes —
//! sprites and box borders are multi-byte, and one wrong count shears a frame.

use crossterm::style::Color;

use super::{Line, Seg};

pub fn seg(s: impl Into<String>, c: Option<Color>) -> Seg {
    (s.into(), c, None)
}

// A key cap as in the design: filled slate box, cyan key.
pub fn chip(key: &str) -> Seg {
    (format!(" {key} "), Some(Color::Cyan), Some(Color::DarkGrey))
}

pub fn plain(s: impl Into<String>) -> Line {
    vec![seg(s, None)]
}

pub fn tinted(s: impl Into<String>, c: Color) -> Line {
    vec![seg(s, Some(c))]
}

pub fn line_w(l: &Line) -> usize {
    l.iter().map(|(s, ..)| s.chars().count()).sum()
}

// Joins two blocks of lines side by side. Every resulting line is padded to
// the same total width — draw_screen centers each line independently, so any
// width variation would shift rows against each other and shear the art.
pub fn beside(left: &[Line], right: &[Line], gap: usize) -> Vec<Line> {
    let lw = left.iter().map(line_w).max().unwrap_or(0);
    let rw = right.iter().map(line_w).max().unwrap_or(0);
    (0..left.len().max(right.len()))
        .map(|i| {
            let mut l: Line = left.get(i).cloned().unwrap_or_default();
            l.push(seg(" ".repeat(lw + gap - line_w(&l)), None));
            if let Some(r) = right.get(i) {
                l.extend(r.iter().cloned());
            }
            let tail = lw + gap + rw - line_w(&l);
            if tail > 0 {
                l.push(seg(" ".repeat(tail), None));
            }
            l
        })
        .collect()
}

// Pads a block of lines to a uniform width so it stays internally aligned
// under draw_screen's per-line centering.
pub fn pad_block(mut lines: Vec<Line>) -> Vec<Line> {
    let w = lines.iter().map(line_w).max().unwrap_or(0);
    for l in &mut lines {
        let pad = w - line_w(l);
        if pad > 0 {
            l.push(seg(" ".repeat(pad), None));
        }
    }
    lines
}

// Truncates a line to exactly `w` chars, padding with spaces when shorter.
pub(super) fn clip_pad(line: &Line, w: usize) -> Line {
    let mut out: Line = Vec::new();
    let mut budget = w;
    for (s, fg, bg) in line {
        if budget == 0 {
            break;
        }
        let t: String = s.chars().take(budget).collect();
        budget -= t.chars().count();
        out.push((t, *fg, *bg));
    }
    if budget > 0 {
        out.push(seg(" ".repeat(budget), None));
    }
    out
}

pub(super) fn center_in(line: &Line, w: usize) -> Line {
    let lw = line_w(line);
    if lw >= w {
        return clip_pad(line, w);
    }
    let lpad = (w - lw) / 2;
    let mut out: Line = vec![seg(" ".repeat(lpad), None)];
    out.extend(line.iter().cloned());
    out.push(seg(" ".repeat(w - lw - lpad), None));
    out
}

// A window of `len` chars starting at `start`, preserving segment colors.
pub(super) fn line_slice(l: &Line, start: usize, len: usize) -> Line {
    let mut out: Line = Vec::new();
    let mut pos = 0usize;
    for (s, fg, bg) in l {
        let seg_start = pos;
        pos += s.chars().count();
        let from = start.max(seg_start);
        let to = (start + len).min(pos);
        if to > from {
            let t: String = s.chars().skip(from - seg_start).take(to - from).collect();
            out.push((t, *fg, *bg));
        }
    }
    out
}

pub(super) fn dim(lines: &[Line]) -> Vec<Line> {
    lines
        .iter()
        .map(|l| l.iter().map(|(s, ..)| (s.clone(), Some(Color::DarkGrey), None)).collect())
        .collect()
}

// Truncates to `w` with a visible … instead of clip_pad's silent cut.
pub(super) fn ellipsize(line: Line, w: usize) -> Line {
    if line_w(&line) <= w {
        return line;
    }
    let mut out = clip_pad(&line, w.saturating_sub(1));
    out.push(seg("…", Some(Color::DarkGrey)));
    out
}

pub(super) fn wrap(text: &str, w: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in text.split_whitespace() {
        let cur = lines.last_mut().unwrap();
        if !cur.is_empty() && cur.chars().count() + 1 + word.chars().count() > w {
            lines.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    lines
}

// wrap() capped at `rows` lines; overflow ends the last visible row with …
pub(super) fn wrapped_text(text: &str, w: usize, rows: usize) -> Vec<Line> {
    let mut wrapped = wrap(text, w);
    if wrapped.len() > rows.max(1) {
        wrapped.truncate(rows.max(1));
        let last = wrapped.last_mut().unwrap();
        let keep: String = last.chars().take(w.saturating_sub(1)).collect();
        *last = format!("{keep}…");
    }
    wrapped.into_iter().map(plain).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_w_sums_segments_by_chars_not_bytes() {
        let l: Line = vec![seg("██", None), seg("ab", None)];
        assert_eq!(line_w(&l), 4);
    }

    // Every joined line must have the SAME width: draw_screen centers lines
    // independently, so any variation shears the blocks apart.
    #[test]
    fn beside_joins_blocks_at_uniform_width() {
        let left = vec![plain("aa"), plain("a")];
        let right = vec![plain("bb"), plain("b"), plain("c")];
        let joined = beside(&left, &right, 2);
        assert_eq!(joined.len(), 3);
        assert!(joined.iter().all(|l| line_w(l) == 2 + 2 + 2));
    }

    #[test]
    fn pad_block_makes_widths_uniform() {
        let block = pad_block(vec![plain("abc"), plain("a")]);
        assert!(block.iter().all(|l| line_w(l) == 3));
    }

    #[test]
    fn center_in_clips_or_centers() {
        assert_eq!(line_w(&center_in(&plain("ab"), 6)), 6);
        assert_eq!(line_w(&center_in(&plain("abcdefgh"), 4)), 4);
    }

    #[test]
    fn line_slice_cuts_across_segments() {
        let l: Line = vec![seg("abc", None), seg("def", Some(Color::Red))];
        assert_eq!(line_slice(&l, 1, 4).iter().map(|(s, ..)| s.as_str()).collect::<String>(), "bcde");
        assert_eq!(line_w(&line_slice(&l, 0, 6)), 6);
        assert_eq!(line_w(&line_slice(&l, 4, 10)), 2);
    }

    #[test]
    fn wrapped_text_marks_truncation_with_ellipsis() {
        let full = wrapped_text("uma frase curta", 40, 3);
        assert_eq!(full.len(), 1);
        let cut = wrapped_text("muitas palavras que não cabem em uma linha só de jeito nenhum", 12, 2);
        assert_eq!(cut.len(), 2);
        let last: String = cut[1].iter().map(|(s, ..)| s.clone()).collect();
        assert!(last.ends_with('…'), "{last:?}");
    }
}
