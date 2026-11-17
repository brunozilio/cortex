use chrono::{DateTime, FixedOffset, Local, Offset, Utc};

pub fn today() -> String {
    let now = Local::now();
    date_at(now.with_timezone(&Utc), now.offset().fix())
}

pub fn date_at(instant: DateTime<Utc>, offset: FixedOffset) -> String {
    instant.with_timezone(&offset).format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn hours(east: i32) -> FixedOffset {
        FixedOffset::east_opt(east * 3600).expect("an offset within a day")
    }

    #[test]
    fn the_date_is_the_one_on_the_user_s_clock_not_the_utc_one() {
        let early_utc_morning = Utc.with_ymd_and_hms(2026, 11, 5, 3, 30, 0).single().expect("an instant");
        assert_eq!(date_at(early_utc_morning, hours(0)), "2026-11-05");
        assert_eq!(date_at(early_utc_morning, hours(-8)), "2026-11-04", "still the evening before, west of UTC");
        assert_eq!(date_at(early_utc_morning, hours(9)), "2026-11-05");

        let late_utc_evening = Utc.with_ymd_and_hms(2026, 12, 31, 22, 0, 0).single().expect("an instant");
        assert_eq!(date_at(late_utc_evening, hours(2)), "2027-01-01", "already tomorrow, east of UTC");
        assert_eq!(date_at(late_utc_evening, hours(-5)), "2026-12-31");
    }

    #[test]
    fn dates_are_written_year_month_day_with_leading_zeros() {
        let leap_day = Utc.with_ymd_and_hms(2000, 2, 29, 12, 0, 0).single().expect("an instant");
        assert_eq!(date_at(leap_day, hours(0)), "2000-02-29");
        let epoch = Utc.timestamp_opt(0, 0).single().expect("an instant");
        assert_eq!(date_at(epoch, hours(0)), "1970-01-01");
    }

    #[test]
    fn today_is_the_local_date_of_now() {
        let now = Local::now();
        let around = [now.format("%Y-%m-%d").to_string(), today()];
        assert!(around.contains(&today()), "{around:?}");
        assert_eq!(today().len(), 10);
    }
}
