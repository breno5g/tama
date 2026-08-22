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

fn schedule_path() -> PathBuf {
    data_dir().join("schedule")
}

// Writes through a temp file and renames over the target: a kill mid-write
// (Android does that to Termux) leaves the previous file intact instead of a
// truncated one. Rename is atomic within the same directory.
fn write_atomic(path: PathBuf, contents: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents)?;
    fs::rename(tmp, path)
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
    write_atomic(state_path(), &serialize(pet))
}

// Everything time-based that used to die with the process. Kept apart from
// the pet save: it changes on its own schedule, and a bad schedule file must
// never risk the pet.
#[derive(Debug, Default, PartialEq)]
pub struct Schedule {
    pub reminders: Vec<(String, u64)>, // (text, epoch)
    pub timer: Option<u64>,            // epoch
    pub pomo: Option<PomoState>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PomoState {
    pub work: u64,
    pub rest: u64,
    pub focus: bool,
    pub until: u64,
    pub cycles: u32,
}

// Anything overdue by more than this was missed while the app was closed:
// firing a two-day-old reminder on startup is noise, not a reminder.
pub const STALE_SECS: u64 = 3600;

pub fn serialize_schedule(s: &Schedule) -> String {
    let mut out = String::new();
    for (text, at) in &s.reminders {
        // one line per reminder: the epoch, a space, then the text
        out.push_str(&format!("remind={at} {}\n", text.replace(['\n', '\r'], " ")));
    }
    if let Some(until) = s.timer {
        out.push_str(&format!("timer={until}\n"));
    }
    if let Some(p) = &s.pomo {
        out.push_str(&format!("pomo={},{},{},{},{}\n", p.work, p.rest, p.focus, p.until, p.cycles));
    }
    out
}

pub fn parse_schedule(s: &str, now: u64) -> Schedule {
    let mut out = Schedule::default();
    let fresh = |at: u64| at + STALE_SECS > now;
    for line in s.lines() {
        let Some((key, val)) = line.split_once('=') else { continue };
        match key {
            "remind" => {
                let Some((at, text)) = val.split_once(' ') else { continue };
                let Ok(at) = at.parse::<u64>() else { continue };
                if fresh(at) {
                    out.reminders.push((text.to_string(), at));
                }
            }
            "timer" => out.timer = val.parse().ok().filter(|at| fresh(*at)),
            "pomo" => {
                let f: Vec<&str> = val.split(',').collect();
                let [work, rest, focus, until, cycles] = f[..] else { continue };
                let (Ok(work), Ok(rest), Ok(until), Ok(cycles)) =
                    (work.parse(), rest.parse(), until.parse(), cycles.parse())
                else {
                    continue;
                };
                out.pomo = Some(PomoState { work, rest, focus: focus == "true", until, cycles });
            }
            _ => {}
        }
    }
    out
}

pub fn load_schedule(now: u64) -> Schedule {
    fs::read_to_string(schedule_path()).map(|s| parse_schedule(&s, now)).unwrap_or_default()
}

pub fn save_schedule(s: &Schedule) -> io::Result<()> {
    write_atomic(schedule_path(), &serialize_schedule(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_round_trips() {
        let s = Schedule {
            reminders: vec![("standup do time".into(), 2000), ("beber água".into(), 2500)],
            timer: Some(3000),
            pomo: Some(PomoState { work: 1500, rest: 300, focus: true, until: 2900, cycles: 2 }),
        };
        assert_eq!(parse_schedule(&serialize_schedule(&s), 1000), s);
    }

    #[test]
    fn schedule_drops_what_went_stale_while_the_app_was_closed() {
        let now = 100_000;
        let s = Schedule {
            reminders: vec![("ontem".into(), now - STALE_SECS - 1), ("agorinha".into(), now - 60)],
            timer: Some(now - STALE_SECS - 1),
            pomo: None,
        };
        let back = parse_schedule(&serialize_schedule(&s), now);
        // the just-missed one still fires; the ancient one is gone
        assert_eq!(back.reminders, vec![("agorinha".to_string(), now - 60)]);
        assert_eq!(back.timer, None);
    }

    #[test]
    fn schedule_survives_garbage_and_newlines_in_text() {
        let s = Schedule {
            reminders: vec![("duas\nlinhas".into(), 500)],
            ..Default::default()
        };
        let back = parse_schedule(&serialize_schedule(&s), 0);
        assert_eq!(back.reminders, vec![("duas linhas".to_string(), 500)]);
        // junk lines are skipped, not fatal
        let back = parse_schedule("lixo\nremind=abc texto\npomo=1,2\ntimer=x\n", 0);
        assert_eq!(back, Schedule::default());
    }

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
