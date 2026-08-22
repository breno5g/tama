//! All user-facing text. One `Strings` struct (see `strings.rs`), one file per
//! locale, picked once at startup.
//!
//! Adding a language: copy `pt_br.rs`, translate the values, add it to `detect`.
//! The compiler then requires every field — a forgotten string cannot ship.
//!
//! Everything on the WIRE stays English (CLI flags, JSON keys, `command`
//! values); only what a person reads goes through here.

mod en;
mod msg;
mod pt_br;
mod strings;

use std::sync::OnceLock;

use crate::assistant::Kind;
use crate::pet::Mood;
use crate::species::Species;

pub use msg::*;
pub use strings::Strings;

static LANG: OnceLock<&'static Strings> = OnceLock::new();

/// The active locale's strings. Resolved on first use and cached.
pub fn t() -> &'static Strings {
    LANG.get_or_init(detect)
}

// TAMA_LANG wins, then the usual POSIX vars — same env convention as
// TAMA_HTTP/TAMA_TOKEN. Anything not English keeps the pt-BR default, so an
// unsupported locale never degrades into a half-translated screen.
fn detect() -> &'static Strings {
    let tag = std::env::var("TAMA_LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if tag.starts_with("en") { &en::S } else { &pt_br::S }
}

// Enum-indexed tables. The arrays follow each enum's declaration order, which
// the tests pin — see `strings.rs`.
pub fn species_name(s: Species) -> &'static str {
    t().species_names[s as usize]
}

pub fn species_trait(s: Species) -> &'static str {
    t().species_traits[s as usize]
}

pub fn species_sound(s: Species) -> &'static str {
    t().species_sounds[s as usize]
}

pub fn species_tastes(s: Species) -> (&'static str, &'static str) {
    t().species_tastes[s as usize]
}

pub fn mood_label(m: Mood) -> &'static str {
    t().mood_labels[m as usize]
}

pub fn kind_label(k: Kind) -> &'static str {
    t().kind_labels[k as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species::SPECIES;

    #[test]
    fn every_locale_labels_every_enum_variant() {
        for s in [&pt_br::S, &en::S] {
            for (i, &sp) in SPECIES.iter().enumerate() {
                assert_eq!(i, sp as usize, "SPECIES order must match the locale tables");
                assert!(!s.species_names[i].is_empty());
                assert!(!s.species_traits[i].is_empty());
                assert!(!s.species_sounds[i].is_empty());
                assert!(!s.species_tastes[i].0.is_empty());
                assert!(!s.species_tastes[i].1.is_empty());
            }
            assert!(s.mood_labels.iter().all(|l| !l.is_empty()));
            assert!(s.kind_labels.iter().all(|l| !l.is_empty()));
            assert!(s.action_labels.iter().all(|l| !l.is_empty()));
            assert!(s.food_names.iter().all(|l| !l.is_empty()));
            // only Happy and Sleeping are warning-free
            assert_eq!(2, s.msg_warnings.iter().filter(|w| w.is_empty()).count());
        }
    }

    #[test]
    fn footers_go_widest_first() {
        // draw_screen picks the first candidate that fits, so a shorter
        // terminal must never get a LONGER footer than a wider one.
        for s in [&pt_br::S, &en::S] {
            for f in [&s.footer_home[..], &s.footer_assistant, &s.footer_ask, &s.footer_input] {
                let widths: Vec<usize> = f.iter().map(|c| c.chars().count()).collect();
                assert!(widths.windows(2).all(|w| w[0] > w[1]), "not descending: {f:?}");
            }
        }
    }

    #[test]
    fn accessors_follow_the_selected_locale() {
        // t() is process-wide and set once, so assert against the tables
        // directly instead of mutating the environment mid-suite.
        assert_eq!("gato", pt_br::S.species_names[Species::Cat as usize]);
        assert_eq!("cat", en::S.species_names[Species::Cat as usize]);
        assert_eq!("dormindo", pt_br::S.mood_labels[Mood::Sleeping as usize]);
        assert_eq!("sleeping", en::S.mood_labels[Mood::Sleeping as usize]);
        assert_eq!("erro", pt_br::S.kind_labels[Kind::Error as usize]);
        assert_eq!("error", en::S.kind_labels[Kind::Error as usize]);
    }
}
