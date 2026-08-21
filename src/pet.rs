use std::time::{SystemTime, UNIX_EPOCH};

use crate::species::Species;

pub const DECAY_SECS: u64 = 30;
pub const MAX_OFFLINE_TICKS: u64 = 24 * 60 * 60 / DECAY_SECS; // cap: 24h of offline decay

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn adj(v: u8, delta: i16) -> u8 {
    (v as i16 + delta).clamp(0, 100) as u8
}

pub struct Food {
    pub hunger: i16,
    pub happiness: i16,
    pub energy: i16,
    pub hygiene: i16,
}

// Index-aligned with i18n::FOOD_NAMES.
pub const FOODS: [Food; 4] = [
    Food { hunger: 15, happiness: 0, energy: 0, hygiene: 0 },
    Food { hunger: 25, happiness: 5, energy: 0, hygiene: 0 },
    Food { hunger: 35, happiness: 0, energy: 0, hygiene: -10 },
    Food { hunger: 0, happiness: 0, energy: 10, hygiene: 0 },
];

#[derive(Debug, Clone, PartialEq)]
pub struct Pet {
    pub species: Species,
    pub name: String,
    pub hunger: u8,    // 100 = fed
    pub happiness: u8, // 100 = happy
    pub energy: u8,    // 100 = rested
    pub hygiene: u8,   // 100 = clean
    pub xp: u32,
    pub level: u32,
    pub born: u64,
    pub last_seen: u64,
    pub zen: bool,
    pub sleeping: bool,    // runtime only, not persisted
    pub tick_parity: bool, // runtime only: hygiene decays every 2nd tick
}

impl Default for Pet {
    fn default() -> Self {
        Pet {
            species: Species::Cat,
            name: String::new(),
            hunger: 80,
            happiness: 90,
            energy: 90,
            hygiene: 80,
            xp: 0,
            level: 1,
            born: now(),
            last_seen: now(),
            zen: false,
            sleeping: false,
            tick_parity: false,
        }
    }
}

impl Pet {
    pub fn tick(&mut self) {
        if self.zen {
            return;
        }
        if self.sleeping {
            self.energy = (self.energy + 5).min(100);
            if self.energy == 100 {
                self.sleeping = false;
            }
            return;
        }
        self.hunger = self.hunger.saturating_sub(1);
        self.energy = self.energy.saturating_sub(1);
        self.tick_parity = !self.tick_parity;
        if self.tick_parity {
            self.hygiene = self.hygiene.saturating_sub(1);
        }
        if self.hunger < 30 || self.energy < 30 || self.hygiene < 30 {
            self.happiness = self.happiness.saturating_sub(1);
        }
    }

    pub fn apply_offline_decay(&mut self) {
        let elapsed = now().saturating_sub(self.last_seen);
        let ticks = (elapsed / DECAY_SECS).min(MAX_OFFLINE_TICKS);
        for _ in 0..ticks {
            self.tick();
        }
        self.last_seen = now();
    }

    pub fn xp_needed(&self) -> u32 {
        self.level * 100
    }

    pub fn gain_xp(&mut self, n: u32) -> bool {
        self.xp += n;
        let mut leveled = false;
        while self.xp >= self.xp_needed() {
            self.xp -= self.xp_needed();
            self.level += 1;
            leveled = true;
        }
        leveled
    }

    pub fn eat(&mut self, food: &Food) -> bool {
        self.hunger = adj(self.hunger, food.hunger);
        self.happiness = adj(self.happiness, food.happiness);
        self.energy = adj(self.energy, food.energy);
        self.hygiene = adj(self.hygiene, food.hygiene);
        self.gain_xp(5)
    }

    pub fn play(&mut self) -> bool {
        self.happiness = adj(self.happiness, 20);
        self.energy = adj(self.energy, -10);
        self.gain_xp(10)
    }

    pub fn bathe(&mut self) -> bool {
        self.hygiene = 100;
        self.gain_xp(5)
    }

    pub fn day(&self) -> u64 {
        now().saturating_sub(self.born) / 86400 + 1
    }

    pub fn mood(&self) -> Mood {
        if self.sleeping {
            Mood::Sleeping
        } else if !self.zen && self.hunger < 30 {
            Mood::Hungry
        } else if !self.zen && self.hygiene < 30 {
            Mood::Dirty
        } else if !self.zen && self.energy < 30 {
            Mood::Sleepy
        } else if !self.zen && self.happiness < 30 {
            Mood::Sad
        } else {
            Mood::Happy
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mood {
    Happy,
    Hungry,
    Dirty,
    Sleepy,
    Sad,
    Sleeping,
}

impl Mood {
    pub fn face(self, blink: bool) -> (char, char) {
        let (eye, mouth) = match self {
            Mood::Happy => ('█', 'w'),
            Mood::Hungry => ('O', 'o'),
            Mood::Dirty => (';', 'o'),
            Mood::Sleepy => ('▄', 'o'),
            Mood::Sad => (';', '~'),
            Mood::Sleeping => ('▄', '.'),
        };
        if blink && self != Mood::Sleeping { ('▄', mouth) } else { (eye, mouth) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_decays_hunger_energy_and_hygiene_on_alternate_ticks() {
        let mut pet = Pet::default();
        pet.tick();
        assert_eq!((pet.hunger, pet.energy, pet.hygiene), (79, 89, 79));
        pet.tick();
        assert_eq!((pet.hunger, pet.energy, pet.hygiene), (78, 88, 79));
    }

    #[test]
    fn happiness_drops_only_when_a_need_is_low() {
        let mut pet = Pet { happiness: 50, ..Pet::default() };
        pet.tick();
        assert_eq!(pet.happiness, 50);
        pet.hunger = 20;
        pet.tick();
        assert_eq!(pet.happiness, 49);
    }

    #[test]
    fn sleeping_tick_recovers_energy_and_wakes_at_full() {
        let mut pet = Pet { sleeping: true, energy: 96, ..Pet::default() };
        pet.tick();
        assert_eq!(pet.energy, 100);
        assert!(!pet.sleeping);
    }

    #[test]
    fn offline_decay_is_capped() {
        let mut pet = Pet { last_seen: 0, ..Pet::default() };
        pet.apply_offline_decay();
        assert_eq!(pet.hunger, 0);
        assert!(pet.hygiene < Pet::default().hygiene);
        assert!(pet.happiness < Pet::default().happiness);
    }

    #[test]
    fn zen_blocks_decay() {
        let mut pet = Pet { zen: true, last_seen: 0, ..Pet::default() };
        let before = pet.clone();
        pet.apply_offline_decay();
        pet.tick();
        assert_eq!(
            (pet.hunger, pet.happiness, pet.energy, pet.hygiene),
            (before.hunger, before.happiness, before.energy, before.hygiene)
        );
    }

    #[test]
    fn xp_levels_up_with_carry() {
        let mut pet = Pet::default();
        assert!(!pet.gain_xp(90));
        assert!(pet.gain_xp(30));
        assert_eq!((pet.level, pet.xp), (2, 20));
        assert_eq!(pet.xp_needed(), 200);
    }

    #[test]
    fn food_applies_effects_and_clamps() {
        let mut pet = Pet { hunger: 90, hygiene: 5, ..Pet::default() };
        pet.eat(&FOODS[2]); // cake: +35 hunger, -10 hygiene
        assert_eq!(pet.hunger, 100);
        assert_eq!(pet.hygiene, 0);
    }

    #[test]
    fn mood_precedence_sleeping_hungry_dirty_sleepy_sad() {
        let mut pet = Pet { hunger: 0, hygiene: 0, energy: 0, happiness: 0, ..Pet::default() };
        pet.sleeping = true;
        assert_eq!(pet.mood(), Mood::Sleeping);
        pet.sleeping = false;
        assert_eq!(pet.mood(), Mood::Hungry);
        pet.hunger = 50;
        assert_eq!(pet.mood(), Mood::Dirty);
        pet.hygiene = 50;
        assert_eq!(pet.mood(), Mood::Sleepy);
        pet.energy = 50;
        assert_eq!(pet.mood(), Mood::Sad);
        pet.happiness = 50;
        assert_eq!(pet.mood(), Mood::Happy);
    }

    #[test]
    fn zen_masks_needy_moods() {
        let pet = Pet { zen: true, hunger: 0, hygiene: 0, energy: 0, happiness: 0, ..Pet::default() };
        assert_eq!(pet.mood(), Mood::Happy);
    }
}
