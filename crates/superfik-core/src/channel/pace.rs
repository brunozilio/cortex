//! How fast a number sends, so it reads as a person and not as a script:
//! "typing…" before each message, a drawn gap between two sends, and the
//! waits before a send that failed is tried again. Times in milliseconds.

pub const TYPING_BASE_MS: u64 = 1_000;
pub const TYPING_PER_CHAR_MS: u64 = 40;
pub const TYPING_MAX_MS: u64 = 8_000;
pub const GAP_MIN_MS: u64 = 2_000;
pub const GAP_MAX_MS: u64 = 6_000;
pub const MAX_ATTEMPTS: u32 = 5;
const RETRY_FIRST_MS: u64 = 30_000;
const RETRY_MAX_MS: u64 = 10 * 60_000;

pub fn typing_ms(text: &str) -> u64 {
    let chars = u64::try_from(text.chars().count()).unwrap_or(u64::MAX);
    TYPING_BASE_MS.saturating_add(TYPING_PER_CHAR_MS.saturating_mul(chars)).min(TYPING_MAX_MS)
}

pub fn gap_ms(draw: f64) -> u64 {
    let draw = if draw.is_nan() { 0.0 } else { draw.clamp(0.0, 0.999_999) };
    let span = (GAP_MAX_MS - GAP_MIN_MS + 1) as f64;
    GAP_MIN_MS + (draw * span).floor() as u64
}

pub fn retry_delay_ms(attempts: u32) -> u64 {
    let doublings = attempts.saturating_sub(1).min(20);
    RETRY_FIRST_MS.saturating_mul(1 << doublings).min(RETRY_MAX_MS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_is_a_second_plus_40_ms_a_character_up_to_eight_seconds() {
        assert_eq!(typing_ms(""), 1_000);
        assert_eq!(typing_ms("Olá!"), 1_160);
        assert_eq!(typing_ms(&"x".repeat(175)), 8_000);
        assert_eq!(typing_ms(&"x".repeat(4_000)), 8_000);
        assert_eq!(typing_ms("👍"), 1_040);
    }

    #[test]
    fn the_gap_is_two_to_six_seconds_from_the_draw() {
        assert_eq!(gap_ms(0.0), GAP_MIN_MS);
        assert_eq!(gap_ms(0.999_999), GAP_MAX_MS);
        assert!(gap_ms(0.5) > GAP_MIN_MS && gap_ms(0.5) < GAP_MAX_MS);
        assert_eq!(gap_ms(-1.0), GAP_MIN_MS);
        assert_eq!(gap_ms(2.0), GAP_MAX_MS);
        assert_eq!(gap_ms(f64::NAN), GAP_MIN_MS);
    }

    #[test]
    fn retries_wait_30_seconds_doubling_up_to_ten_minutes() {
        let waits: Vec<u64> = [1, 2, 3, 4, 5, 6, 30].into_iter().map(retry_delay_ms).collect();
        assert_eq!(waits, [30_000, 60_000, 120_000, 240_000, 480_000, 600_000, 600_000]);
        assert_eq!(retry_delay_ms(0), 30_000);
    }
}
