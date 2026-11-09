//! How a run reaches a model Anthropic does not serve: the
//! app's gateway, a loopback server speaking Anthropic's Messages API, and
//! the token that admits this run alone. Filled by the backend for each run
//! and never sent over the wire.

#[derive(Clone, PartialEq, Eq)]
pub struct GatewayAccess {
    pub base_url: String,
    pub token: String,
}

impl GatewayAccess {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Self {
        Self { base_url: base_url.into(), token: token.into() }
    }
}

impl std::fmt::Debug for GatewayAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayAccess").field("base_url", &self.base_url).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_token() {
        let access = GatewayAccess::new("http://127.0.0.1:4100", "secret-run-token");
        assert!(!format!("{access:?}").contains("secret-run-token"));
    }
}
