//! Sprite data: the pixel/LCD art for every species, in both sizes, plus
//! the one-line faces. Pure data — the rendering lives in the parent module.

use super::{ArtSize, Species};

impl Species {
    // `%` = eye, `&` = mouth — swapped per mood, so one template serves all moods.
    // Pixel/LCD sprite style (tamagotchi-like), drawn with block elements.
    pub(super) fn template(self, size: ArtSize) -> &'static str {
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

    pub(super) fn tiny_face(self) -> &'static str {
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
