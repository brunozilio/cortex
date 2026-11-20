pub const STALL_MS: u64 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stall {
    pub step: String,
    pub for_ms: u64,
}

impl Stall {
    pub fn describe(&self) -> String {
        format!("The screen stopped responding for {:.1}s while it was in {}.", self.for_ms as f64 / 1000.0, self.step)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Watchdog {
    step: String,
    since: u64,
    reported: bool,
}

impl Watchdog {
    pub fn enter(&mut self, now_ms: u64, step: &str) -> Option<Stall> {
        let took = now_ms.saturating_sub(self.since);
        let froze =
            (took >= STALL_MS && !self.step.is_empty()).then(|| Stall { step: self.step.clone(), for_ms: took });
        self.step = step.to_owned();
        self.since = now_ms;
        self.reported = false;
        froze
    }

    pub fn step(&self) -> &str {
        &self.step
    }

    pub fn pause(&mut self) {
        self.step.clear();
    }

    pub fn check(&mut self, now_ms: u64) -> Option<Stall> {
        let took = now_ms.saturating_sub(self.since);
        if self.reported || self.step.is_empty() || took < STALL_MS {
            return None;
        }
        self.reported = true;
        Some(Stall { step: self.step.clone(), for_ms: took })
    }
}

#[cfg(test)]
mod tests;
