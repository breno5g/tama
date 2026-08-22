//! How the pet looks while it talks: the per-kind expression map and the
//! per-kind animations. The animations are dimension-preserving on purpose —
//! success hops into a reserved top row, error shakes inside a reserved side
//! column — so an animating pet never reflows the screen around it.

use crossterm::style::Color;

use crate::assistant::Kind;

pub fn kind_color(kind: Kind) -> Color {
    match kind {
        Kind::Info => Color::Cyan,
        Kind::Success => Color::Green,
        Kind::Warn => Color::Yellow,
        Kind::Error => Color::Red,
    }
}

// The pet reacts to what it is saying, per the design's expression map:
// info = calm, success = happy, warn = wide eyes (no blink), error = sad.
pub fn kind_face(kind: Kind, frame: usize) -> (char, char) {
    let eye = if frame % 4 == 3 { '▄' } else { '█' };
    match kind {
        Kind::Info => (eye, '.'),
        Kind::Success => (eye, 'w'),
        Kind::Warn => ('O', 'o'),
        Kind::Error => (';', '~'),
    }
}

// The design's per-kind animations, dimension-preserving so nothing reflows:
// success hops using the reserved top row; error shakes inside a reserved
// side column; the rest stay still.
pub(super) fn animate_art(mut art: Vec<String>, kind: Kind, frame: usize) -> Vec<String> {
    match kind {
        Kind::Success => {
            if frame % 2 == 1 {
                art.remove(0);
                let w = art.last().map(|l| l.chars().count()).unwrap_or(0);
                art.push(" ".repeat(w));
            }
        }
        Kind::Error => {
            let left = frame % 2 == 0;
            for l in art.iter_mut() {
                if left {
                    l.insert(0, ' ');
                } else {
                    l.push(' ');
                }
            }
        }
        _ => {}
    }
    art
}

pub(super) fn animate_tiny(face: String, kind: Kind, frame: usize) -> String {
    if kind == Kind::Error {
        if frame % 2 == 0 { format!(" {face}") } else { format!("{face} ") }
    } else {
        face
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::ArtSize;

    #[test]
    fn kind_faces_are_distinct_and_warn_never_blinks() {
        let kinds = [Kind::Info, Kind::Success, Kind::Warn, Kind::Error];
        for (i, a) in kinds.iter().enumerate() {
            for b in kinds.iter().skip(i + 1) {
                assert_ne!(kind_face(*a, 0), kind_face(*b, 0), "{a:?} vs {b:?}");
            }
        }
        assert_eq!(kind_face(Kind::Warn, 3), kind_face(Kind::Warn, 0));
        assert_ne!(kind_face(Kind::Info, 3).0, kind_face(Kind::Info, 0).0); // blinks
    }

    // Hop and shake must not change the art's footprint on any frame.
    #[test]
    fn kind_animations_preserve_dimensions() {
        use crate::species::{render_art_face, Species};
        for kind in [Kind::Info, Kind::Success, Kind::Warn, Kind::Error] {
            let mut shapes = Vec::new();
            for frame in 0..4 {
                let (eye, mouth) = kind_face(kind, frame);
                let art = animate_art(render_art_face(Species::Dragon, ArtSize::Large, eye, mouth), kind, frame);
                let w = art[0].chars().count();
                assert!(art.iter().all(|l| l.chars().count() == w), "{kind:?} misaligned at frame {frame}");
                shapes.push((art.len(), w));
            }
            assert!(shapes.windows(2).all(|p| p[0] == p[1]), "{kind:?} footprint changed across frames");
        }
    }
}
