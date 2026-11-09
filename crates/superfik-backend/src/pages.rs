//! Browser cards' pages on the server (ADR 0010): the seam a page host
//! fills. The host owns each page for the principal that opened it, and
//! answers through the outbox it was handed, with `ServerEvent::Page`
//! reports and `ServerEvent::PageFrame`s.

use superfik_core::Principal;
use superfik_core::page::{PageCall, PageReport};
use superfik_protocol::ServerEvent;

use crate::Outbox;

pub trait PageService: Send + Sync {
    fn call(&self, principal: &Principal, call: PageCall, answer: Outbox);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoPages;

impl PageService for NoPages {
    fn call(&self, _principal: &Principal, call: PageCall, answer: Outbox) {
        if let PageCall::Open { id, url, .. } = call {
            let error = "this server has no browser".to_owned();
            answer.send(ServerEvent::Page { browser: id, report: PageReport::Failed { url, error } });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use superfik_core::BrowserId;
    use superfik_core::page::Profile;

    use super::*;

    #[test]
    fn without_a_browser_opening_a_page_fails_and_the_rest_is_ignored() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let outbox = Outbox::new(tx, Arc::new(|| {}));
        let id = BrowserId::new();
        let open =
            PageCall::Open { id, url: "https://a.test".into(), profile: Profile::Ephemeral, size: (1, 1), scale: 1.0 };
        NoPages.call(&Principal::LocalOwner, open, outbox.clone());
        NoPages.call(&Principal::LocalOwner, PageCall::Reload(id), outbox);
        let events: Vec<ServerEvent> = rx.try_iter().collect();
        assert!(
            matches!(&events[..], [ServerEvent::Page { browser, report: PageReport::Failed { .. } }] if *browser == id)
        );
    }
}
