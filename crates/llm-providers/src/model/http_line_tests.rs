use std::time::Duration;

use super::{STREAM_IDLE, idle_from};

#[test]
fn the_stream_idle_deadline_is_configurable_and_falls_back_to_its_default() {
    assert_eq!(idle_from(Some("45")), Duration::from_secs(45));
    assert_eq!(idle_from(Some(" 7 ")), Duration::from_secs(7));
    for unusable in [None, Some(""), Some("0"), Some("soon"), Some("-3")] {
        assert_eq!(idle_from(unusable), STREAM_IDLE, "{unusable:?}");
    }
}
