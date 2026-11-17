pub const MAX_SWITCHES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Overloaded,
    Unauthorized,
    Network,
    Provider,
    TooLong,
}

impl Failure {
    pub fn moves_down_the_chain(self) -> bool {
        matches!(self, Self::Overloaded | Self::Unauthorized | Self::Network)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Overloaded => "overloaded",
            Self::Unauthorized => "unauthorized",
            Self::Network => "network",
            Self::Provider => "provider",
            Self::TooLong => "too_long",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackChoice {
    Pass,
    Forward(String),
    Stop,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Fallback {
    chain: Vec<String>,
    tried: Vec<String>,
}

impl Fallback {
    pub fn new(chain: Vec<String>) -> Self {
        Self { chain, tried: Vec::new() }
    }

    pub fn chain(&self) -> &[String] {
        &self.chain
    }

    pub fn tried(&self) -> &[String] {
        &self.tried
    }

    pub fn failed(&mut self, current: &str) {
        if !self.tried.iter().any(|model| model == current) {
            self.tried.push(current.to_owned());
        }
    }

    pub fn next(&mut self, current: &str, failure: Failure, verdict: FallbackChoice) -> Option<String> {
        self.failed(current);
        if self.tried.len() > MAX_SWITCHES {
            return None;
        }
        let untried = |model: &String| !self.tried.contains(model);
        match verdict {
            FallbackChoice::Stop => None,
            FallbackChoice::Forward(model) => untried(&model).then_some(model),
            FallbackChoice::Pass if failure.moves_down_the_chain() => {
                self.chain.iter().find(|model| untried(model)).cloned()
            }
            FallbackChoice::Pass => None,
        }
    }

    pub fn succeeded(&mut self) {
        self.tried.clear();
    }
}

#[cfg(test)]
mod tests;
