use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct Degraded {
    reason: Option<Arc<str>>,
    told: Arc<AtomicBool>,
}

impl Degraded {
    pub fn because(reason: impl AsRef<str>) -> Self {
        Self { reason: Some(Arc::from(reason.as_ref())), told: Arc::default() }
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }

    pub fn told_once(&self, say: impl FnOnce(&str) -> String) -> Option<String> {
        let reason = self.reason.as_deref()?;
        (!self.told.swap(true, Ordering::SeqCst)).then(|| say(reason))
    }
}

pub fn database_notice(reason: &str) -> String {
    format!("The session list and memory could not be opened ({reason}); they will not be kept after this session.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_degraded_is_said_once_however_many_hold_it_and_nothing_is_said_when_all_is_well() {
        let degraded = Degraded::because("app.db was written by a newer version");
        let sibling = degraded.clone();
        assert_eq!(
            degraded.told_once(database_notice).as_deref(),
            Some(
                "The session list and memory could not be opened (app.db was written by a newer version); \
                 they will not be kept after this session."
            )
        );
        assert_eq!(sibling.told_once(database_notice), None, "once per process, not once per client");
        assert_eq!(sibling.reason(), Some("app.db was written by a newer version"));
        assert_eq!(Degraded::default().told_once(database_notice), None);
    }
}
