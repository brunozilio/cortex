mod cleanup;
mod filters;
mod invoked;
mod limit;
mod savings;

pub use cleanup::{PLAIN_ENV, clean};
pub use filters::{Filtered, filter};
pub use invoked::program;
pub use limit::{CUT_NOTE, DEFAULT_LIMIT, LARGEST_LIMIT, OutputLimit, SMALLEST_LIMIT};
pub use savings::{Saving, report};

use crate::features::{Features, OUTPUT_CLEANUP, OUTPUT_FILTERS, OUTPUT_SAVINGS};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shaping {
    pub cleanup: bool,
    pub filters: bool,
    pub savings: bool,
}

impl From<&Features> for Shaping {
    fn from(features: &Features) -> Self {
        Self {
            cleanup: features.on(OUTPUT_CLEANUP),
            filters: features.on(OUTPUT_FILTERS),
            savings: features.on(OUTPUT_SAVINGS),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_step_follows_its_own_flag() {
        assert_eq!(Shaping::default(), Shaping { cleanup: false, filters: false, savings: false });
        assert_eq!(Shaping::from(&Features::default()), Shaping { cleanup: true, filters: true, savings: true });
        let features = Features::default().layered(&json!({ OUTPUT_FILTERS: false }), true).expect("flags");
        assert_eq!(Shaping::from(&features), Shaping { cleanup: true, filters: false, savings: true });
    }
}
