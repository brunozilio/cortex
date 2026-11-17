use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{Delta, ModelApi, ModelError, Reply, TurnRequest};

pub struct Pooled {
    accounts: Vec<(String, Arc<dyn ModelApi>)>,
    current: AtomicUsize,
}

impl std::fmt::Debug for Pooled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pooled").field("accounts", &self.accounts.len()).finish_non_exhaustive()
    }
}

impl Pooled {
    pub fn new(accounts: Vec<(String, Arc<dyn ModelApi>)>) -> Self {
        Self { accounts, current: AtomicUsize::new(0) }
    }
}

impl ModelApi for Pooled {
    fn turn(&self, request: &TurnRequest, on: &mut dyn FnMut(Delta), stop: &AtomicBool) -> Result<Reply, ModelError> {
        let count = self.accounts.len();
        let start = self.current.load(Ordering::Relaxed);
        let mut last = Err(ModelError::Unauthorized("no account is connected".into()));
        for offset in 0..count {
            let at = (start + offset) % count;
            let mut wrote = false;
            let mut seen = |delta: Delta| {
                wrote = true;
                on(delta);
            };
            last = self.accounts[at].1.turn(request, &mut seen, stop);
            let hands_over =
                last.as_ref().err().and_then(ModelError::failure).is_some_and(domain::account_pool::hands_over);
            if !hands_over || wrote {
                self.current.store(at, Ordering::Relaxed);
                return last;
            }
            if let Err(error) = &last {
                tracing::warn!(%error, account = %self.accounts[at].0, "the account is busy or signed out");
            }
            if offset + 1 < count {
                let to = self.accounts[(at + 1) % count].0.clone();
                on(Delta::HandedOver { from: self.accounts[at].0.clone(), to });
            }
        }
        last
    }
}

#[cfg(test)]
#[path = "pooled_tests.rs"]
mod tests;
