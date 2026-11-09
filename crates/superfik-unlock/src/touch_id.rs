//! Touch ID through LocalAuthentication: the system's own sheet, which
//! falls back to the login password when there is no sensor or it fails.

#![allow(unsafe_code)]

use std::time::Duration;

use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_local_authentication::{LAContext, LAPolicy};

use crate::{Unlock, UnlockError};

const WAIT: Duration = Duration::from_secs(120);

#[derive(Debug, Default)]
pub struct TouchId;

impl Unlock for TouchId {
    fn unlock(&self, reason: &str) -> Result<(), UnlockError> {
        let policy = LAPolicy::DeviceOwnerAuthentication;
        // SAFETY: `+[LAContext new]` has no preconditions; it returns an
        let context = unsafe { LAContext::new() };
        // SAFETY: `context` is a valid, retained LAContext and `policy` is a
        if unsafe { context.canEvaluatePolicy_error(policy) }.is_err() {
            return Err(UnlockError::Unavailable);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let reply = RcBlock::new(move |ok: Bool, _error: *mut NSError| {
            let _ = tx.send(ok.as_bool());
        });
        // SAFETY: `context` is a valid LAContext that outlives the call; the
        unsafe { context.evaluatePolicy_localizedReason_reply(policy, &NSString::from_str(reason), &reply) };
        match rx.recv_timeout(WAIT) {
            Ok(true) => Ok(()),
            _ => Err(UnlockError::Cancelled),
        }
    }
}
