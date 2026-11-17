use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, Weak};
use std::time::{Duration, Instant};

use crate::api::RequestId;
use platform::sync::Locked;

use crate::handler::outbox::Replies;

const RELEASE_WAIT: Duration = Duration::from_secs(5);
const RELEASE_POLL: Duration = Duration::from_millis(10);

#[derive(Debug, Default)]
pub(crate) struct Flows {
    stops: Mutex<HashMap<RequestId, Weak<AtomicBool>>>,
}

impl Flows {
    pub fn began(&self, replies: &Replies) {
        let mut stops = self.stops.locked();
        stops.retain(|_, stop| stop.strong_count() > 0);
        stops.insert(replies.id(), std::sync::Arc::downgrade(replies.stop()));
    }

    pub fn cancel(&self, request: RequestId) -> bool {
        let Some(flow) = self.stops.locked().remove(&request) else { return false };
        let Some(stop) = flow.upgrade() else { return false };
        stop.store(true, Ordering::SeqCst);
        drop(stop);
        let asked = Instant::now();
        while flow.strong_count() > 0 && asked.elapsed() < RELEASE_WAIT {
            std::thread::sleep(RELEASE_POLL);
        }
        true
    }
}
