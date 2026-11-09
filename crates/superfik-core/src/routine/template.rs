//! Ready-made routines for what a product needs after it ships: keeping
//! production healthy, announcing what changed, watching the revenue. A
//! template fills the form; the member adjusts it and picks the accounts.

use super::{Autonomy, RoutineDraft, Schedule};

const NINE: u16 = 9 * 60;
const FIVE_PM: u16 = 17 * 60;
const SIX_PM: u16 = 18 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Template {
    ProductionUpkeep,
    ReleaseNotes,
    RevenueDigest,
    CustomerSupport,
}

impl Template {
    pub const ALL: [Template; 4] =
        [Template::CustomerSupport, Template::ProductionUpkeep, Template::ReleaseNotes, Template::RevenueDigest];

    pub const fn plugins(self) -> &'static [&'static str] {
        match self {
            Self::ProductionUpkeep => &["sentry", "linear", "github"],
            Self::ReleaseNotes => &["github"],
            Self::RevenueDigest => &["stripe"],
            Self::CustomerSupport => &["whatsapp", "email", "linear", "github"],
        }
    }

    pub const fn answers(self) -> bool {
        matches!(self, Self::CustomerSupport)
    }

    pub fn draft(self, name: String, utc_offset: i32) -> RoutineDraft {
        let (schedule, autonomy, brief) = match self {
            Self::ProductionUpkeep => (
                Schedule::Daily { minute: NINE, utc_offset },
                Autonomy::Bounded,
                "Check the error tracker for issues that are new or regressed since the last run. For each one: \
                 find the cause in the code, then open an issue in the project's tracker (Linear, else GitHub \
                 Issues) with the error, the cause and a proposed fix, unless one is already open. When the fix \
                 is small and safe, make it with a test and commit it. List every issue with its link, its cause \
                 and what you did.",
            ),
            Self::ReleaseNotes => (
                Schedule::Weekly { weekday: 4, minute: FIVE_PM, utc_offset },
                Autonomy::Draft,
                "Read the pull requests merged since the last run. Write release notes for users in plain words, \
                 grouped as new, improved and fixed, and a short post announcing the highlights for X and \
                 LinkedIn. Publish nothing: put both in your report.",
            ),
            Self::RevenueDigest => (
                Schedule::Weekly { weekday: 0, minute: NINE, utc_offset },
                Autonomy::Draft,
                "Using the payments account, summarise the past week: new and lost customers, recurring revenue \
                 and its change, failed payments that need attention, and anything unusual. Short, with numbers.",
            ),
            Self::CustomerSupport => (
                Schedule::Daily { minute: SIX_PM, utc_offset },
                Autonomy::Draft,
                "You answer the product's customers. When a message arrives, read the conversation and answer \
                 in the customer's language, briefly and kindly, from what the code, the docs and the project's \
                 memory say; never promise dates or refunds. When it is a bug, look for its cause in the code \
                 and open an issue (Linear, else GitHub Issues) unless one is open, and tell the customer it is \
                 being looked at. On the daily run, list the day's conversations: who wrote, what they needed, \
                 what is still open.",
            ),
        };
        RoutineDraft {
            name,
            brief: brief.to_owned(),
            schedule,
            autonomy,
            model: None,
            connections: Vec::new(),
            answers: self.answers(),
        }
    }
}
