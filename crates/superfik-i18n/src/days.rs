//! Day-group labels: "Today", "Yesterday", the weekday within the last
//! week, else the date (with the year when it is not the current one).

use superfik_core::DayGroup;
use superfik_core::days::{civil, local_day, weekday};

use crate::Catalog;

pub(crate) fn label(catalog: &dyn Catalog, group: DayGroup, now_unix: i64, utc_offset_secs: i32) -> String {
    let day = match group {
        DayGroup::Today => return catalog.today().to_owned(),
        DayGroup::Yesterday => return catalog.yesterday().to_owned(),
        DayGroup::Day(day) => day,
    };
    let today = local_day(now_unix, utc_offset_secs);
    if (2..7).contains(&(today - day)) {
        return catalog.weekday(weekday(day)).to_owned();
    }
    let (year, month, date) = civil(day);
    catalog.date(month, date, (year != civil(today).0).then_some(year))
}
