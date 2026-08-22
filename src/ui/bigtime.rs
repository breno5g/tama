//! The big LCD clock digits, tty-clock style. Pure bitmap rendering: give it
//! "24:59" and it hands back 5 rows of block art.

// 3×5 pixel bitmaps for the big LCD clock, tty-clock style; each pixel
// renders as a double-width "██" block so digits read square on screen.
// Each row is 3 bits, most significant bit = left column.
const DIGIT_BITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b001, 0b001, 0b001], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

// Renders "24:59" as 5 rows of block art. Unknown chars are skipped.
pub fn big_time(text: &str) -> Vec<String> {
    let mut rows = vec![String::new(); 5];
    for ch in text.chars() {
        for (r, row) in rows.iter_mut().enumerate() {
            match ch {
                '0'..='9' => {
                    let bits = DIGIT_BITS[ch as usize - '0' as usize][r];
                    for c in [2u8, 1, 0] {
                        row.push_str(if bits >> c & 1 == 1 { "██" } else { "  " });
                    }
                    row.push(' ');
                }
                ':' => row.push_str(if r == 1 || r == 3 { "██ " } else { "   " }),
                _ => {}
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn big_time_renders_uniform_rows_and_distinct_digits() {
        let art = big_time("25:09");
        assert_eq!(art.len(), 5);
        let w = art[0].chars().count();
        assert_eq!(w, 4 * 7 + 3); // 4 digits + colon
        assert!(art.iter().all(|l| l.chars().count() == w));
        for a in 0..10u8 {
            for b in (a + 1)..10 {
                assert_ne!(big_time(&a.to_string()), big_time(&b.to_string()), "{a} vs {b}");
            }
        }
        assert!(big_time("x").iter().all(|l| l.is_empty())); // unknown chars skipped
    }
}
