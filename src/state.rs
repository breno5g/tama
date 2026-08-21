use std::fs;
use std::io;
use std::path::PathBuf;

use crate::pet::{now, Pet};
use crate::species::Species;

pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".local/share/tama")
}

fn state_path() -> PathBuf {
    data_dir().join("state")
}

pub fn input_path() -> PathBuf {
    data_dir().join("input")
}

pub fn output_path() -> PathBuf {
    data_dir().join("output")
}

pub fn serialize(pet: &Pet) -> String {
    format!(
        "species={}\nname={}\nhunger={}\nhappiness={}\nenergy={}\nhygiene={}\nxp={}\nlevel={}\nborn={}\nlast_seen={}\nzen={}\n",
        pet.species.id(),
        pet.name,
        pet.hunger,
        pet.happiness,
        pet.energy,
        pet.hygiene,
        pet.xp,
        pet.level,
        pet.born,
        pet.last_seen,
        pet.zen
    )
}

// Unknown keys and unparsable values fall back to defaults, so old or
// hand-edited state files never crash the app.
pub fn parse(s: &str) -> Pet {
    let mut pet = Pet::default();
    for line in s.lines() {
        let Some((key, val)) = line.split_once('=') else { continue };
        match key {
            "species" => pet.species = Species::from_id(val).unwrap_or(pet.species),
            "name" => pet.name = val.to_string(),
            "hunger" => pet.hunger = val.parse().unwrap_or(pet.hunger),
            "happiness" => pet.happiness = val.parse().unwrap_or(pet.happiness),
            "energy" => pet.energy = val.parse().unwrap_or(pet.energy),
            "hygiene" => pet.hygiene = val.parse().unwrap_or(pet.hygiene),
            "xp" => pet.xp = val.parse().unwrap_or(pet.xp),
            "level" => pet.level = val.parse().unwrap_or(pet.level),
            "born" => pet.born = val.parse().unwrap_or(pet.born),
            "last_seen" => pet.last_seen = val.parse().unwrap_or(pet.last_seen),
            "zen" => pet.zen = val == "true",
            _ => {}
        }
    }
    pet
}

pub fn load() -> Option<Pet> {
    let mut pet = parse(&fs::read_to_string(state_path()).ok()?);
    pet.apply_offline_decay();
    Some(pet)
}

pub fn save(pet: &mut Pet) -> io::Result<()> {
    pet.last_seen = now();
    let path = state_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, serialize(pet))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip() {
        let pet = Pet {
            species: Species::Dragon,
            name: "rex da silva".to_string(),
            hunger: 42,
            happiness: 7,
            energy: 100,
            hygiene: 55,
            xp: 340,
            level: 3,
            born: 99,
            last_seen: 123,
            zen: true,
            sleeping: false,
            tick_parity: false,
        };
        assert_eq!(parse(&serialize(&pet)), pet);
    }

    #[test]
    fn parse_garbage_falls_back_to_default() {
        let pet = parse("not a state file\nhunger=abc\nunknown=1");
        assert_eq!(pet.hunger, Pet::default().hunger);
        assert_eq!(pet.level, 1);
    }

    #[test]
    fn parse_partial_file_keeps_defaults_for_missing_fields() {
        let pet = parse("species=dog\nhunger=10\n");
        assert_eq!(pet.species, Species::Dog);
        assert_eq!(pet.hunger, 10);
        assert_eq!(pet.hygiene, Pet::default().hygiene);
        assert!(pet.name.is_empty());
    }
}
