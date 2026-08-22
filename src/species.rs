use crate::pet::Mood;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Species {
    Cat,
    Dog,
    Bunny,
    Dragon,
    Ghost,
    Frog,
    Owl,
    Fox,
    Penguin,
    Octopus,
}

pub const SPECIES: [Species; 10] = [
    Species::Cat,
    Species::Dog,
    Species::Bunny,
    Species::Dragon,
    Species::Ghost,
    Species::Frog,
    Species::Owl,
    Species::Fox,
    Species::Penguin,
    Species::Octopus,
];

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
            Species::Frog => "frog",
            Species::Owl => "owl",
            Species::Fox => "fox",
            Species::Penguin => "penguin",
            Species::Octopus => "octopus",
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
    ▄█▄         ▄█▄
   █▀ ▀█▄▄▄▄▄▄▄█▀ ▀█
  █▀  ▀▀  ▀▀▀  ▀▀  ▀█
 =█    %       %    █=
 =█                 █=
  █        ▄        █
  ▀█▄     ▀&▀     ▄█▀
   ▄█▀▀▀▀▀▀▀▀▀▀▀▀▀█▄
  █▀ ▀▀▄  ▀▀▀  ▄▀▀ ▀█
  █▄▄▄▄█ █▄▄▄█ █▄▄▄▄█
   ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀"#,
            (Species::Cat, ArtSize::Small) => r#"
 ▄█▄     ▄█▄
█▀ ▀█▄▄▄█▀ ▀█
█ ▀ %   % ▀ █
█    ▀&▀    █
▀█▄       ▄█▀
  ▀▀▀▀▀▀▀▀▀"#,
            (Species::Dog, ArtSize::Large) => r#"
    ▄▄▄▀▀▀▀▀▀▀▀▀▀▀▄▄▄
   ██▀ ▄▄▄▄▄▄▄▄▄▄▄ ▀██
   ██  █         █  ██
   ██  █  %   %  █  ██
   ██  █         █  ██
   ▀██ █  ▄▄▄▄▄  █ ██▀
     ▀ █▄█ ▄▄▄ █▄█ ▀
       ██  ▀█▀  ██
       ██   &   ██
        █▄▄▄▄▄▄▄█
        ▄█▀▀▀▀▀█▄
        █ ▄▄ ▄▄ █
        ▀▀▀▀▀▀▀▀▀"#,
            (Species::Dog, ArtSize::Small) => r#"
 ▄▄▀▀▀▀▀▀▀▄▄
██  %   %  ██
██    ▄    ██
▀██  ▀█▀  ██▀
  █   &   █
  ▀▄▄▄▄▄▄▄▀"#,
            (Species::Bunny, ArtSize::Large) => r#"
     ▄█▀█▄     ▄█▀█▄
     █ ▀ █     █ ▀ █
     █ ▀ █     █ ▀ █
     █ ▀ █     █ ▀ █
     ▀█▄██▄▄▄▄▄██▄█▀
     ▄▀           ▀▄
   =█   %       %   █=
   =█               █=
    █       ▄       █
    █      ▀&▀      █
     █     ▀▀▀     █
      ▀▄▄▄▄▄▄▄▄▄▄▄▀
       ▄█▄     ▄█▄"#,
            (Species::Bunny, ArtSize::Small) => r#"
 ▄█▄     ▄█▄
 █ █     █ █
 █ █     █ █
▄▀▀▀▀▀▀▀▀▀▀▀▄
█  %     %  █
█    ▀&▀    █
▀▄▄▄▄▄▄▄▄▄▄▄▀"#,
            (Species::Dragon, ArtSize::Large) => r#"
   █▄                 ▄█
   ▀██▄▄           ▄▄██▀
     ▀▄██▄▄▀▀▀▀▀▄▄██▄▀
      █             █
    ▄█▀  %       %  ▀█▄
   ██                 ██
   ██     ▄▄▄▄▄▄▄     ██
   █▀█   █  ▄ ▄  █   █▀█
   █ ▀▄  █   &   █  ▄▀ █
   ▀▄▄ ▀▄▀█▀▀▀▀█▀▄▀ ▄▄▀
      █  ▄▄   ▄▄  █▄
      ▀▀▀▀▀▀▀▀▀▀▀▀ ▀▄▄o"#,
            (Species::Dragon, ArtSize::Small) => r#"
█▄           ▄█
 ▀█▄▄▀▀▀▀▀▄▄█▀
  █  %   %  █
 ██   ▄▄▄   ██
 █▀█ █ & █ █▀█
  ▀▄▄▀▀▀▀▀▄▄▀ ▄o"#,
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
            (Species::Frog, ArtSize::Large) => r#"
   ▄▀▀▀▄   ▄▀▀▀▄
   █ % █   █ % █
  ▄█▄▄▄▀▀▀▀▀▄▄▄█▄
  █             █
  █      &      █
  █  ▀▄▄▄▄▄▄▄▀  █
  ▀▄▄▄▄▄▄▄▄▄▄▄▄▄▀
  ▄█▀▄▄       ▄▄▀█▄"#,
            (Species::Frog, ArtSize::Small) => r#"
 ▄▀▀▀▄ ▄▀▀▀▄
 █ % █ █ % █
▄█▄▄▄▀▀▀▄▄▄█▄
█     &     █
▀▄▄▄▄▄▄▄▄▄▄▄▀"#,
            (Species::Owl, ArtSize::Large) => r#"
 ▄▀▄▄▄▄▄▄▄▄▄▄▄▀▄
 █  ▄▄▄   ▄▄▄  █
 █ ( % ) ( % ) █
 █  ▀▀▀ ▄ ▀▀▀  █
 █     ▀&▀     █
 ▀█  ▀▄▄▄▄▄▀  █▀
  █ ▄▄▄   ▄▄▄ █
  █▄▄▄▄▄▄▄▄▄▄▄█
    ▀█▀   ▀█▀"#,
            (Species::Owl, ArtSize::Small) => r#"
▄▀▄▄▄▄▄▄▄▀▄
█ (%) (%) █
█    ▄    █
█   ▀&▀   █
█▄▄▄▄▄▄▄▄▄█
  ▀█▀ ▀█▀"#,
            (Species::Fox, ArtSize::Large) => r#"
  ▄█▄             ▄█▄
 █▀ ▀█▄▄▄▄▄▄▄▄▄▄▄█▀ ▀█
 █▀▀▀  ▀       ▀  ▀▀▀█
==█   %         %   █==
  ▀█▄               ▄█▀
    ▀█▄    ▄▄▄    ▄█▀
      ▀██▄ ▀█▀ ▄██▀
        ▀██ & ██▀
          ▀▀▀▀▀"#,
            (Species::Fox, ArtSize::Small) => r#"
 ▄█▄     ▄█▄
█▀ ▀█▄▄▄█▀ ▀█
█  %     %  █
▀█▄  ▄▄▄  ▄█▀
  ▀█▄▀&▀▄█▀
    ▀▀▀▀▀"#,
            (Species::Penguin, ArtSize::Large) => r#"
    ▄▄█▀▀▀▀▀█▄▄
   █▀  %   %  ▀█
   █      ▄     █
   █     ▀&▀    █
  █▀ ▄▀▀▀▀▀▀▀▄ ▀█
  █  █       █  █
  █▄ █       █ ▄█
   ▀▄▀▄▄▄▄▄▄▄▀▄▀
    ▄█▄     ▄█▄"#,
            (Species::Penguin, ArtSize::Small) => r#"
 ▄█▀▀▀▀▀█▄
█  %   %  █
█    ▄    █
█   ▀&▀   █
█ ▄▀▀▀▀▀▄ █
▀▄█▄▄▄▄▄█▄▀"#,
            (Species::Octopus, ArtSize::Large) => r#"
    ▄█▀▀▀▀▀▀▀█▄
   █  %     %  █
   █           █
   █     ▄     █
   █    ▀&▀    █
   ▀█▄▄▄▄▄▄▄▄▄█▀
   ▄▀▄ █▀▄ ▄▀█ ▄▀▄
   █ ▀▀▀ ▀▀▀ ▀▀▀ █"#,
            (Species::Octopus, ArtSize::Small) => r#"
 ▄█▀▀▀▀▀█▄
█  %   %  █
█    &    █
▀█▄▄▄▄▄▄▄█▀
▄▀▄ █▀▄ ▄▀▄
█ ▀▀▀ ▀▀▀ █"#,
        }
    }

    fn tiny_face(self) -> &'static str {
        match self {
            Species::Cat => "(=^ %&% ^=)",
            Species::Dog => "∩( %&% )∩",
            Species::Bunny => r"\\( %&% )//",
            Species::Dragon => "<{ %&% }>",
            Species::Ghost => "~( %&% )~",
            Species::Frog => "o( %&% )o",
            Species::Owl => "(( %&% ))",
            Species::Fox => "=( %&% )=",
            Species::Penguin => "d( %&% )b",
            Species::Octopus => "}( %&% ){",
        }
    }
}

pub fn zzz(frame: usize) -> &'static str {
    ["z    ", "z Z  ", "z Z z"][frame % 3]
}

// Lines are padded to a uniform width so per-line centering keeps the block aligned.
fn render_lines(species: Species, size: ArtSize, eye: char, mouth: char, zzz_frame: Option<usize>) -> Vec<String> {
    let art = species.template(size).replace('%', &eye.to_string()).replace('&', &mouth.to_string());
    let mut lines: Vec<String> = art.lines().skip(1).map(String::from).collect();
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    for l in &mut lines {
        let pad = width - l.chars().count();
        l.push_str(&" ".repeat(pad));
    }
    // The zzz row is ALWAYS reserved (blank while awake) so falling asleep
    // never changes the art height — a sleep toggle must not reflow the layout.
    let zline = match zzz_frame {
        Some(frame) => format!("{:>width$}", zzz(frame)),
        None => " ".repeat(width),
    };
    lines.insert(0, zline);
    lines
}

pub fn render_art(species: Species, mood: Mood, frame: usize, size: ArtSize) -> Vec<String> {
    let (eye, mouth) = mood.face(frame % 4 == 3); // blink 1 in 4 frames
    render_lines(species, size, eye, mouth, (mood == Mood::Sleeping).then_some(frame))
}

// Art with an explicit face — the assistant's per-message-kind expressions.
pub fn render_art_face(species: Species, size: ArtSize, eye: char, mouth: char) -> Vec<String> {
    render_lines(species, size, eye, mouth, None)
}

pub fn render_tiny(species: Species, mood: Mood, frame: usize) -> String {
    let (eye, mouth) = mood.face(frame % 4 == 3);
    render_tiny_face(species, eye, mouth)
}

pub fn render_tiny_face(species: Species, eye: char, mouth: char) -> String {
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
    fn every_template_has_exactly_two_eyes_and_one_mouth() {
        for species in SPECIES {
            for size in [ArtSize::Large, ArtSize::Small] {
                let t = species.template(size);
                assert_eq!(t.matches('%').count(), 2, "{species:?}/{size:?} eyes");
                assert_eq!(t.matches('&').count(), 1, "{species:?}/{size:?} mouth");
            }
            let f = species.tiny_face();
            assert_eq!(f.matches('%').count(), 2, "{species:?} tiny eyes");
            assert_eq!(f.matches('&').count(), 1, "{species:?} tiny mouth");
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
    fn tiny_face_has_no_trailing_reserve() {
        let face = render_tiny(Species::Bunny, Mood::Happy, 0);
        assert_eq!(face, face.trim_end());
    }

    #[test]
    fn species_ids_round_trip() {
        for s in SPECIES {
            assert_eq!(Species::from_id(s.id()), Some(s));
        }
        assert_eq!(Species::from_id("unicorn"), None);
    }
}
