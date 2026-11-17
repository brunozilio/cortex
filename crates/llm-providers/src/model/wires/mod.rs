pub mod chat;
pub mod messages;
pub mod responses;

use std::sync::Arc;

use domain::ModelOption;

use crate::model::ModelApi;
use crate::model::http::Http;
#[cfg(feature = "subscription-providers")]
use crate::providers::{antigravity, kiro};

type Open = fn(Arc<dyn Http>, Option<&ModelOption>) -> Arc<dyn ModelApi>;

const ALL: &[(&str, Open)] = &[
    (chat::NAME, chat::open),
    (messages::NAME, messages::open),
    (responses::NAME, responses::open),
    #[cfg(feature = "subscription-providers")]
    (kiro::wire::NAME, kiro::wire::open),
    #[cfg(feature = "subscription-providers")]
    (antigravity::wire::NAME, antigravity::wire::open),
];

pub fn open(name: &str, http: Arc<dyn Http>, model: Option<&ModelOption>) -> Option<Arc<dyn ModelApi>> {
    ALL.iter().find(|(known, _)| *known == name).map(|(_, open)| open(http, model))
}

#[cfg(test)]
mod tests {
    use super::ALL;

    #[cfg(feature = "subscription-providers")]
    #[test]
    fn the_registered_wires_are_these_in_this_order() {
        let names: Vec<&str> = ALL.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["chat", "messages", "responses", "event_stream", "cloud_code"]);
    }

    #[cfg(not(feature = "subscription-providers"))]
    #[test]
    fn the_registered_wires_are_these_in_this_order() {
        let names: Vec<&str> = ALL.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["chat", "messages", "responses"]);
    }
}
