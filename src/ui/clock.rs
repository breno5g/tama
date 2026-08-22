//! Wall-clock time for the header, cached so the render loop never shells out
//! on every frame.

use std::time::{Duration, Instant};

pub struct Clock {
    text: String,
    hour: u8,
    fetched: Instant,
}

impl Clock {
    pub fn new() -> Self {
        let (text, hour) = fetch_clock();
        Clock { text, hour, fetched: Instant::now() }
    }

    pub fn get(&mut self) -> (String, u8) {
        if self.fetched.elapsed() > Duration::from_secs(20) {
            let (text, hour) = fetch_clock();
            self.text = text;
            self.hour = hour;
            self.fetched = Instant::now();
        }
        (self.text.clone(), self.hour)
    }
}

// ponytail: local time via the `date` binary — std has no timezone support
// and a chrono dependency is not worth one HH:MM string. Cached for 20s.
fn fetch_clock() -> (String, u8) {
    std::process::Command::new("date")
        .arg("+%H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 5)
        .map(|s| {
            let hour = s[..2].parse().unwrap_or(12);
            (s, hour)
        })
        .unwrap_or_else(|| ("--:--".to_string(), 12))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_falls_back_gracefully() {
        let (text, hour) = fetch_clock();
        assert!(text == "--:--" || text.len() == 5);
        assert!(hour < 24);
    }
}
