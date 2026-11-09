//! LinkedIn's Community Management answers into protocol-free messages.
//! Pure.
//!
//! Each top-level comment on one of the organization's posts is a
//! conversation (`comment:<comment urn>`), its replies joining it; the
//! organization's own comments are the agent's when it wrote them, else a
//! member's. `feed` is the page itself: sending there publishes a post.

use serde_json::Value;
use superfik_core::channel::{Message, Source};

pub const COMMENT: &str = "comment:";
pub const FEED: &str = "feed";

pub fn posts(body: &Value) -> Vec<String> {
    body["elements"].as_array().into_iter().flatten().filter_map(|p| p["id"].as_str().map(str::to_owned)).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commented {
    pub message: Message,
    pub post: String,
}

pub fn comments(body: &Value, post: &str, organization: &str, ours: impl Fn(&str) -> bool) -> Vec<Commented> {
    let mut found: Vec<Commented> = body["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|comment| {
            let id = comment["$URN"].as_str().or(comment["commentUrn"].as_str())?;
            let actor = comment["actor"].as_str()?;
            let text = comment["message"]["text"].as_str().filter(|t| !t.is_empty())?;
            let thread = comment["parentComment"].as_str().unwrap_or(id);
            let source = match actor == organization {
                true if ours(id) => Source::Agent,
                true => Source::Member,
                false => Source::Contact,
            };
            let message = Message {
                id: id.to_owned(),
                exchange: format!("{COMMENT}{thread}"),
                is_group: true,
                sender: Some(actor.to_owned()),
                sender_name: None,
                text: text.to_owned(),
                media: None,
                mentioned_me: true,
                reply_to_me: false,
                at: comment["created"]["time"].as_i64().unwrap_or_default(),
                source,
            };
            Some(Commented { message, post: comment["object"].as_str().unwrap_or(post).to_owned() })
        })
        .collect();
    found.sort_by_key(|c| c.message.at);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORG: &str = "urn:li:organization:2414183";
    const POST: &str = "urn:li:share:7250000000000000000";

    fn fixture(name: &str) -> Value {
        let path = format!("{}/tests/fixtures/linkedin/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&std::fs::read(path).expect("fixture")).expect("json")
    }

    #[test]
    fn the_pages_posts_are_listed_by_urn() {
        assert_eq!(posts(&fixture("posts.json")), [POST, "urn:li:ugcPost:7240000000000000000"]);
    }

    #[test]
    fn a_comment_starts_a_conversation_and_replies_join_it() {
        let found = comments(&fixture("comments.json"), POST, ORG, |_| false);
        assert_eq!(found.len(), 2);
        let (first, reply) = (&found[0].message, &found[1].message);
        assert_eq!(first.exchange, "comment:urn:li:comment:(urn:li:activity:7250000000000000001,7250000000000000100)");
        assert_eq!(first.source, Source::Contact);
        assert_eq!(first.sender.as_deref(), Some("urn:li:person:Ab1Cd2"));
        assert_eq!(reply.exchange, first.exchange);
        assert_eq!(reply.source, Source::Member, "the page's own reply, not the agent's");
        assert_eq!(found[0].post, POST);
    }

    #[test]
    fn the_agents_own_reply_is_its_own() {
        let mine = "urn:li:comment:(urn:li:activity:7250000000000000001,7250000000000000101)";
        let found = comments(&fixture("comments.json"), POST, ORG, |id| id == mine);
        assert_eq!(found[1].message.source, Source::Agent);
    }
}
