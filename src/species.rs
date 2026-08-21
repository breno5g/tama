use crate::pet::Mood;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Species {
    Cat,
    Dog,
    Bunny,
    Dragon,
    Ghost,
}

pub const SPECIES: [Species; 5] = [Species::Cat, Species::Dog, Species::Bunny, Species::Dragon, Species::Ghost];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ArtSize {
    Large,
    Small,
}

impl Species {
    pub fn id(self) -> &'static str {
        match self {
            Species::Cat => "cat",
            Species::Dog => "dog",
            Species::Bunny => "bunny",
            Species::Dragon => "dragon",
            Species::Ghost => "ghost",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        SPECIES.into_iter().find(|s| s.id() == id)
    }

    // `%` = eye, `&` = mouth — swapped per mood, so one template serves all moods.
    // Pixel/LCD sprite style (tamagotchi-like), drawn with block elements.
    fn template(self, size: ArtSize) -> &'static str {
        match (self, size) {
            (Species::Cat, ArtSize::Large) => r#"
   ▄█▄           ▄█▄
  █▀ ▀█▄▄▄▄▄▄▄▄▄█▀ ▀█
 █▀                 ▀█
 █                   █
 █    %         %    █
 █                   █
 █         ▄         █
 ▀█▄      ▀&▀      ▄█▀
   ▀█▄▄▄▄▄▄▄▄▄▄▄▄▄█▀
   ▄█▀▀▀▀▀▀▀▀▀▀▀▀▀█▄
  █▀    ▄     ▄    ▀█   ▄▄
  █▄▄▄▄█ █▄▄▄█ █▄▄▄▄█▄▄█ █
   ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀"#,
            (Species::Cat, ArtSize::Small) => r#"
 ▄█▄       ▄█▄
█▀ ▀█▄▄▄▄▄█▀ ▀█
█  %       %  █
█     ▀&▀     █
▀█▄         ▄█▀
  ▀▀▀▀▀▀▀▀▀▀▀"#,
            (Species::Dog, ArtSize::Large) => r#"
   ▄▄▄▄         ▄▄▄▄
  █▀▀▀▀█▄▄▄▄▄▄▄█▀▀▀▀█
  █   █▀       ▀█   █
  █   █  %   %  █   █
  ▀█▄▄█         █▄▄█▀
      █    ▄    █
      █   ▀&▀   █
      ▀█▄     ▄█▀
       ▄█▀▀▀▀▀█▄
      █▀       ▀█
      █ ▄▄   ▄▄ █
      ▀▀▀▀▀▀▀▀▀▀▀"#,
            (Species::Dog, ArtSize::Small) => r#"
▄▄▄▄       ▄▄▄▄
█▀▀█▄▄▄▄▄▄▄█▀▀█
█ █ %     % █ █
▀▄█    &    █▄▀
  █▄       ▄█
   ▀▀▀▀▀▀▀▀▀"#,
            (Species::Bunny, ArtSize::Large) => r#"
    ▄█▄       ▄█▄
    █ █       █ █
    █ █       █ █
    █ ▀█▄▄▄▄▄█▀ █
   ▄▀           ▀▄
   █   %     %   █
   █      ▄      █
   █     ▀&▀     █
   ▀█▄         ▄█▀
   ▄█▀▀▀▀▀▀▀▀▀▀▀█▄
  █▀             ▀█
  █▄▄█▀█▄▄▄▄▄█▀█▄▄█"#,
            (Species::Bunny, ArtSize::Small) => r#"
  █▄     ▄█
  █ █   █ █
  █ ▀▄▄▄▀ █
 █▀ %   % ▀█
 █    &    █
  ▀▄▄▄▄▄▄▄▀"#,
            (Species::Dragon, ArtSize::Large) => r#"
  ▄▄             ▄▄
  █ ▀▄▄▀▀▀▀▀▀▀▄▄▀ █
  ▀▄▄█         █▄▄▀
    █  %     %  █
   ▄█     ▄     █▄
  █▀ █   ▀&▀   █ ▀█
  █  ▀█▄     ▄█▀  █
  █▄▄ ▄█▀▀▀▀▀█▄ ▄▄█
   ▀ █▀       ▀█ ▀
     █ ▄▄   ▄▄ █▄▄
     ▀▀▀▀▀▀▀▀▀▀▀ ▀▄▄o"#,
            (Species::Dragon, ArtSize::Small) => r#"
 ▄▄ ▄▄▄▄▄ ▄▄
 █▄█▀▀▀▀▀█▄█
 █ %     % █
█▀   ▀&▀   ▀█
█▄▄▄     ▄▄▄█
   ▀▀▀▀▀▀▀"#,
            (Species::Ghost, ArtSize::Large) => r#"
    ▄▄█▀▀▀▀▀█▄▄
   █▀         ▀█
  █▀           ▀█
  █   %     %   █
  █             █
  █      &      █
  █             █
  █             █
  █▄▀█▄▀█▄▀█▄▀█▄█"#,
            (Species::Ghost, ArtSize::Small) => r#"
  ▄█▀▀▀▀▀█▄
 █  %   %  █
 █    &    █
 █         █
 █▄▀█▄▀█▄▀█▄"#,
        }
    }

    fn tiny_face(self) -> &'static str {
        match self {
            Species::Cat => "(=^ %&% ^=)",
            Species::Dog => "(v. %&% .v)",
            Species::Bunny => r"\\( %&% )//",
            Species::Dragon => "<{ %&% }>",
            Species::Ghost => "~( %&% )~",
        }
    }
}

pub fn zzz(frame: usize) -> &'static str {
    ["z    ", "z Z  ", "z Z z"][frame % 3]
}

// Lines are padded to a uniform width so per-line centering keeps the block aligned.
pub fn render_art(species: Species, mood: Mood, frame: usize, size: ArtSize) -> Vec<String> {
    let (eye, mouth) = mood.face(frame % 4 == 3); // blink 1 in 4 frames
    let art = species.template(size).replace('%', &eye.to_string()).replace('&', &mouth.to_string());
    let mut lines: Vec<String> = art.lines().skip(1).map(String::from).collect();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    for l in &mut lines {
        let pad = width - l.chars().count();
        l.push_str(&" ".repeat(pad));
    }
    // The zzz row is ALWAYS reserved (blank while awake) so falling asleep
    // never changes the art height — a sleep toggle must not reflow the layout.
    let zline = if mood == Mood::Sleeping { format!("{:>width$}", zzz(frame)) } else { " ".repeat(width) };
    lines.insert(0, zline);
    lines
}

// Width is constant across moods by construction (only single chars swap);
// the sleeping zzz is NOT embedded here — a caller that shows it appends it
// in a reserved trailing slot so it never shifts what sits next to the face.
pub fn render_tiny(species: Species, mood: Mood, frame: usize) -> String {
    let (eye, mouth) = mood.face(frame % 4 == 3);
    species.tiny_face().replace('%', &eye.to_string()).replace('&', &mouth.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOODS: [Mood; 6] = [Mood::Happy, Mood::Hungry, Mood::Dirty, Mood::Sleepy, Mood::Sad, Mood::Sleeping];

    #[test]
    fn every_species_renders_every_mood_aligned() {
        for species in SPECIES {
            for mood in MOODS {
                for size in [ArtSize::Large, ArtSize::Small] {
                    for frame in 0..4 {
                        let lines = render_art(species, mood, frame, size);
                        assert!(!lines.is_empty());
                        let w = lines[0].chars().count();
                        assert!(lines.iter().all(|l| l.chars().count() == w), "{species:?}/{mood:?}/{size:?} misaligned");
                        assert!(lines.iter().all(|l| !l.contains('%') && !l.contains('&')));
                    }
                }
            }
        }
    }

    #[test]
    fn tiny_faces_render_every_mood() {
        for species in SPECIES {
            for mood in MOODS {
                for frame in 0..4 {
                    let face = render_tiny(species, mood, frame);
                    assert!(!face.contains('%') && !face.contains('&'), "{species:?}/{mood:?}");
                }
            }
        }
    }

    #[test]
    fn sleeping_adds_zzz() {
        let art = render_art(Species::Cat, Mood::Sleeping, 2, ArtSize::Small);
        assert!(art[0].contains("z Z z"));
    }

    #[test]
    fn tiny_face_has_no_trailing_reserve() {
        let face = render_tiny(Species::Bunny, Mood::Happy, 0);
        assert_eq!(face, face.trim_end());
    }

    // Falling asleep must not change the art's footprint — the layout would
    // reflow otherwise.
    #[test]
    fn sleeping_keeps_art_dimensions_constant() {
        for species in SPECIES {
            for size in [ArtSize::Large, ArtSize::Small] {
                let awake = render_art(species, Mood::Happy, 0, size);
                let asleep = render_art(species, Mood::Sleeping, 0, size);
                assert_eq!(awake.len(), asleep.len(), "{species:?}/{size:?} height changed");
                assert_eq!(awake[0].chars().count(), asleep[0].chars().count());
            }
            let tiny_awake = render_tiny(species, Mood::Happy, 0);
            let tiny_asleep = render_tiny(species, Mood::Sleeping, 0);
            assert_eq!(tiny_awake.chars().count(), tiny_asleep.chars().count(), "{species:?} tiny width changed");
        }
    }

    #[test]
    fn species_ids_round_trip() {
        for s in SPECIES {
            assert_eq!(Species::from_id(s.id()), Some(s));
        }
        assert_eq!(Species::from_id("unicorn"), None);
    }
}
