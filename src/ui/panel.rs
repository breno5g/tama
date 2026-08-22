//! Boxes and overlays: the titled panels of the interface, and the modal
//! splicing that keeps the screen behind a dialog visible but dimmed.

use crossterm::style::Color;

use super::line::{clip_pad, line_slice, line_w, pad_block, seg};
use super::Line;

// A titled box, as in the Interface 2.0 design: ┌─ title ────┐ … └────┘.
pub(super) fn panel(title: &str, body: &[Line], w: usize) -> Vec<Line> {
    boxed(Some((title, Color::DarkGrey)), Color::DarkGrey, body, w)
}

pub(super) fn boxed(title: Option<(&str, Color)>, border_color: Color, body: &[Line], w: usize) -> Vec<Line> {
    let border = Some(border_color);
    let inner = w.saturating_sub(4);
    let top = match title {
        Some((title, title_color)) => {
            let title: String = title.chars().take(w.saturating_sub(6)).collect();
            let dash = w.saturating_sub(title.chars().count() + 5);
            vec![
                seg("┌─ ", border),
                seg(title, Some(title_color)),
                seg(format!(" {}┐", "─".repeat(dash)), border),
            ]
        }
        None => vec![seg(format!("┌{}┐", "─".repeat(w.saturating_sub(2))), border)],
    };
    let mut out: Vec<Line> = vec![top];
    for b in body {
        let mut l: Line = vec![seg("│ ", border)];
        l.extend(clip_pad(b, inner));
        l.push(seg(" │", border));
        out.push(l);
    }
    out.push(vec![seg(format!("└{}┘", "─".repeat(w.saturating_sub(2))), border)]);
    out
}

// Splices a modal block over the center of a backdrop, per the design's
// overlay: the screen behind stays visible, dimmed. Falls back to the modal
// alone when the backdrop is too small to hold it.
pub(super) fn overlay(base: Vec<Line>, over: &[Line]) -> Vec<Line> {
    let mut base = pad_block(base);
    let bw = base.iter().map(line_w).max().unwrap_or(0);
    let ow = over.iter().map(line_w).max().unwrap_or(0);
    if ow > bw || over.len() > base.len() {
        return over.to_vec();
    }
    let top = (base.len() - over.len()) / 2;
    let left = (bw - ow) / 2;
    for (i, o) in over.iter().enumerate() {
        let row = &base[top + i];
        let mut composed = line_slice(row, 0, left);
        composed.extend(clip_pad(o, ow));
        composed.extend(line_slice(row, left + ow, bw - left - ow));
        base[top + i] = composed;
    }
    base
}

#[cfg(test)]
mod tests {
    use super::super::line::plain;
    use super::*;

    #[test]
    fn panel_has_uniform_width_and_borders() {
        let p = panel("status", &[plain("hi"), plain("a much longer line that overflows")], 20);
        assert_eq!(p.len(), 4);
        assert!(p.iter().all(|l| line_w(l) == 20));
        assert_eq!(p[0][0].0, "┌─ ");
        assert_eq!(p[0][1].0, "status");
        assert!(p[3][0].0.starts_with('└'));
    }

    // The modal must sit centered over the backdrop with the backdrop intact
    // around it — total dimensions unchanged.
    #[test]
    fn overlay_centers_modal_and_keeps_backdrop_dimensions() {
        let base: Vec<Line> = (0..9).map(|_| plain("##########")).collect();
        let modal: Vec<Line> = (0..3).map(|_| plain("XXXX")).collect();
        let out = overlay(base, &modal);
        assert_eq!(out.len(), 9);
        assert!(out.iter().all(|l| line_w(l) == 10));
        let mid: String = out[4].iter().map(|(s, ..)| s.as_str()).collect();
        assert_eq!(mid, "###XXXX###");
        let top: String = out[0].iter().map(|(s, ..)| s.as_str()).collect();
        assert_eq!(top, "##########");
    }

    #[test]
    fn overlay_too_big_falls_back_to_modal_alone() {
        let base: Vec<Line> = vec![plain("##")];
        let modal: Vec<Line> = vec![plain("XXXX"), plain("XXXX")];
        assert_eq!(overlay(base, &modal).len(), 2);
    }
}
