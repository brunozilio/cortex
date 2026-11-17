use jiff::tz::Offset;
use jiff::{Timestamp, Zoned};

pub fn today() -> String {
    let now = Zoned::now();
    date_at(now.timestamp(), now.offset())
}

pub fn date_at(instant: Timestamp, offset: Offset) -> String {
    instant.to_zoned(offset.to_time_zone()).strftime("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hours(east: i8) -> Offset {
        Offset::from_hours(east).expect("an offset within a day")
    }

    fn utc(text: &str) -> Timestamp {
        text.parse().expect("an instant")
    }

    #[test]
    fn the_date_is_the_one_on_the_user_s_clock_not_the_utc_one() {
        let early_utc_morning = utc("2026-11-05T03:30:00Z");
        assert_eq!(date_at(early_utc_morning, hours(0)), "2026-11-05");
        assert_eq!(date_at(early_utc_morning, hours(-8)), "2026-11-04", "still the evening before, west of UTC");
        assert_eq!(date_at(early_utc_morning, hours(9)), "2026-11-05");

        let late_utc_evening = utc("2026-12-31T22:00:00Z");
        assert_eq!(date_at(late_utc_evening, hours(2)), "2027-01-01", "already tomorrow, east of UTC");
        assert_eq!(date_at(late_utc_evening, hours(-5)), "2026-12-31");
    }

    #[test]
    fn dates_are_written_year_month_day_with_leading_zeros() {
        let leap_day = utc("2000-02-29T12:00:00Z");
        assert_eq!(date_at(leap_day, hours(0)), "2000-02-29");
        assert_eq!(date_at(Timestamp::UNIX_EPOCH, hours(0)), "1970-01-01");
    }

    #[test]
    fn today_is_the_local_date_of_now() {
        let now = Zoned::now();
        let around = [now.strftime("%Y-%m-%d").to_string(), today()];
        assert!(around.contains(&today()), "{around:?}");
        assert_eq!(today().len(), 10);
    }
}
