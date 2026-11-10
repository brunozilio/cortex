//! Who a run's tools act as on other services (ADR 0011): the member who
//! sent it, never the server. Filled by the server for each run and never
//! sent over the wire, so a client cannot set it and logs never show it.

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RunCredentials {
    pub owner: Option<crate::Principal>,
    pub github: Option<String>,
    pub model: Option<crate::provider::ProviderKey>,
    pub model_guard: Option<ModelGuard>,
    /// The host's in-process MCP servers (the SDK's tools in code).
    pub mcp: Vec<crate::mcp::LocalMcp>,
    /// Which client connection the run belongs to (its background commands
    /// end with that client); 0 when unknown.
    pub client: u64,
}

type CheckModel = dyn Fn(&crate::ModelOption) -> Result<(), String> + Send + Sync;
#[derive(Clone)]
pub struct ModelGuard(std::sync::Arc<CheckModel>);
impl ModelGuard {
    pub fn new(check: impl Fn(&crate::ModelOption) -> Result<(), String> + Send + Sync + 'static) -> Self {
        Self(std::sync::Arc::new(check))
    }
    pub fn check(&self, model: &crate::ModelOption) -> Result<(), String> {
        (self.0)(model)
    }
}
impl std::fmt::Debug for ModelGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ModelGuard(..)")
    }
}
impl PartialEq for ModelGuard {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for ModelGuard {}

impl std::fmt::Debug for RunCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunCredentials")
            .field("github", &self.github.is_some())
            .field("model", &self.model.is_some())
            .field("mcp", &self.mcp)
            .finish()
    }
}

impl RunCredentials {
    pub fn env(&self) -> Vec<(String, String)> {
        self.github.as_deref().map(github_env).unwrap_or_default()
    }
}

pub fn github_env(token: &str) -> Vec<(String, String)> {
    let basic = base64(&format!("x-access-token:{token}"));
    vec![
        ("GH_TOKEN".to_owned(), token.to_owned()),
        ("GIT_CONFIG_COUNT".to_owned(), "1".to_owned()),
        ("GIT_CONFIG_KEY_0".to_owned(), "http.https://github.com/.extraheader".to_owned()),
        ("GIT_CONFIG_VALUE_0".to_owned(), format!("AUTHORIZATION: basic {basic}")),
    ]
}

fn base64(text: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard() {
        assert_eq!(base64(""), "");
        assert_eq!(base64("f"), "Zg==");
        assert_eq!(base64("fo"), "Zm8=");
        assert_eq!(base64("foo"), "Zm9v");
        assert_eq!(base64("x-access-token:t"), "eC1hY2Nlc3MtdG9rZW46dA==");
    }

    #[test]
    fn a_member_signed_in_to_github_reaches_it_and_no_one_else_does() {
        assert!(RunCredentials::default().env().is_empty());
        let env = RunCredentials { github: Some("ghu_ana".into()), model: None, ..Default::default() }.env();
        assert!(env.contains(&("GH_TOKEN".to_owned(), "ghu_ana".to_owned())));
        assert!(
            env.iter().any(|(key, value)| key == "GIT_CONFIG_VALUE_0" && value.starts_with("AUTHORIZATION: basic "))
        );
        assert!(
            !format!("{:?}", RunCredentials { github: Some("ghu_ana".into()), model: None, ..Default::default() })
                .contains("ghu_ana")
        );
    }

    #[test]
    fn the_model_key_never_prints_nor_reaches_the_environment() {
        let credentials = RunCredentials {
            github: None,
            model: Some(crate::provider::ProviderKey::new("sk-ant-secret")),
            ..Default::default()
        };
        assert!(!format!("{credentials:?}").contains("secret"));
        assert!(credentials.env().is_empty(), "the native engine reads it; no process needs it");
    }
}
