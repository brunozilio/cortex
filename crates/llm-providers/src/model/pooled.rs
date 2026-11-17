use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{BoxFuture, CancellationToken, Delta, ModelApi, ModelError, OnDelta, Reply, TurnRequest};

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
    fn turn<'a>(
        &'a self,
        request: &'a TurnRequest,
        on: OnDelta<'a>,
        cancel: &'a CancellationToken,
    ) -> BoxFuture<'a, Result<Reply, ModelError>> {
        Box::pin(self.handed_round(request, on, cancel))
    }
}

impl Pooled {
    async fn handed_round(
        &self,
        request: &TurnRequest,
        on: OnDelta<'_>,
        cancel: &CancellationToken,
    ) -> Result<Reply, ModelError> {
        let count = self.accounts.len();
        let start = self.current.load(Ordering::Relaxed);
        let mut last = Err(ModelError::Unauthorized("no account is connected".into()));
        for offset in 0..count {
            let at = (start + offset) % count;
            let mut wrote = false;
            last = {
                let mut seen = |delta: Delta| {
                    wrote = true;
                    on(delta);
                };
                self.accounts[at].1.turn(request, &mut seen, cancel).await
            };
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
