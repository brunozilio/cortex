use serde::{Deserialize, Serialize};
use serde_json::Value;

const SKEW: i64 = 60;

fn split(url: &str) -> Option<(String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let (host, path) = rest.split_once('/').map_or((rest, ""), |(host, path)| (host, path));
    if host.is_empty() {
        return None;
    }
    let path = path.trim_end_matches('/');
    Some((format!("{scheme}://{host}"), if path.is_empty() { String::new() } else { format!("/{path}") }))
}

pub fn resource_metadata_urls(server: &str) -> Vec<String> {
    let Some((origin, path)) = split(server) else { return Vec::new() };
    let mut urls = vec![format!("{origin}/.well-known/oauth-protected-resource{path}")];
    if !path.is_empty() {
        urls.push(format!("{origin}/.well-known/oauth-protected-resource"));
    }
    urls
}

pub fn auth_server_metadata_urls(issuer: &str) -> Vec<String> {
    let Some((origin, path)) = split(issuer) else { return Vec::new() };
    let mut urls = vec![
        format!("{origin}/.well-known/oauth-authorization-server{path}"),
        format!("{origin}/.well-known/openid-configuration{path}"),
    ];
    if !path.is_empty() {
        urls.push(format!("{origin}{path}/.well-known/openid-configuration"));
    }
    urls
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AuthServer {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub registration_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

#[derive(Debug)]
pub struct Authorize<'a> {
    pub server: &'a AuthServer,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub challenge: &'a str,
    pub state: &'a str,
    pub resource: &'a str,
}

impl Authorize<'_> {
    pub fn url(&self) -> String {
        let mut pairs = vec![
            ("response_type", "code".to_owned()),
            ("client_id", self.client_id.to_owned()),
            ("redirect_uri", self.redirect_uri.to_owned()),
            ("code_challenge", self.challenge.to_owned()),
            ("code_challenge_method", "S256".to_owned()),
            ("state", self.state.to_owned()),
            ("resource", self.resource.to_owned()),
        ];
        if !self.server.scopes_supported.is_empty() {
            pairs.push(("scope", self.server.scopes_supported.join(" ")));
        }
        let joiner = if self.server.authorization_endpoint.contains('?') { '&' } else { '?' };
        format!("{}{joiner}{}", self.server.authorization_endpoint, form(&pairs))
    }
}

pub fn form(pairs: &[(&str, String)]) -> String {
    pairs.iter().map(|(key, value)| format!("{}={}", encode(key), encode(value))).collect::<Vec<_>>().join("&")
}

fn encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (byte as char).to_string(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

pub fn endpoint_ok(url: &str) -> bool {
    let Some(host) = crate::permission_rules::site(url) else { return false };
    let loopback = host == "localhost" || host == "::1" || host.starts_with("127.");
    match url.split_once("://").map(|(scheme, _)| scheme.to_ascii_lowercase()).as_deref() {
        Some("https") => loopback || !crate::permission_rules::is_private(&host),
        Some("http") => loopback,
        _ => false,
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub token_endpoint: String,
    pub resource: String,
}

impl std::fmt::Debug for McpTokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTokens")
            .field("client_id", &self.client_id)
            .field("expires_at", &self.expires_at)
            .field("token_endpoint", &self.token_endpoint)
            .field("resource", &self.resource)
            .finish_non_exhaustive()
    }
}

impl McpTokens {
    pub fn answered(&self, answer: &Value, now: i64) -> Result<Self, String> {
        let access = answer["access_token"].as_str().ok_or_else(|| {
            answer["error_description"].as_str().or(answer["error"].as_str()).unwrap_or("no access token").to_owned()
        })?;
        Ok(Self {
            access_token: access.to_owned(),
            refresh_token: answer["refresh_token"].as_str().map(str::to_owned).or_else(|| self.refresh_token.clone()),
            expires_at: answer["expires_in"].as_i64().map(|seconds| now + seconds),
            ..self.clone()
        })
    }

    pub fn fresh(&self, now: i64) -> bool {
        self.expires_at.is_none_or(|at| now + SKEW < at)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn metadata_is_looked_for_at_the_well_known_places() {
        assert_eq!(
            resource_metadata_urls("https://mcp.linear.app/mcp?x=1"),
            [
                "https://mcp.linear.app/.well-known/oauth-protected-resource/mcp",
                "https://mcp.linear.app/.well-known/oauth-protected-resource"
            ]
        );
        assert_eq!(
            auth_server_metadata_urls("https://auth.test"),
            [
                "https://auth.test/.well-known/oauth-authorization-server",
                "https://auth.test/.well-known/openid-configuration"
            ]
        );
        assert_eq!(auth_server_metadata_urls("https://auth.test/tenant").len(), 3);
    }

    #[test]
    fn the_authorization_url_carries_pkce_state_and_the_resource() {
        let server = AuthServer {
            authorization_endpoint: "https://auth.test/authorize".into(),
            token_endpoint: "https://auth.test/token".into(),
            registration_endpoint: None,
            scopes_supported: vec!["read".into(), "write".into()],
        };
        let url = Authorize {
            server: &server,
            client_id: "c1",
            redirect_uri: "http://127.0.0.1:5555/callback",
            challenge: "abc",
            state: "s",
            resource: "https://mcp.test/mcp",
        }
        .url();
        assert_eq!(
            url,
            "https://auth.test/authorize?response_type=code&client_id=c1&redirect_uri=http%3A%2F%2F127.0.0.1%3A5555%2Fcallback\
             &code_challenge=abc&code_challenge_method=S256&state=s&resource=https%3A%2F%2Fmcp.test%2Fmcp&scope=read%20write"
        );
    }

    #[test]
    fn tokens_expire_and_a_refresh_keeps_the_refresh_token() {
        let kept = McpTokens {
            access_token: "old".into(),
            refresh_token: Some("r1".into()),
            expires_at: None,
            client_id: "c1".into(),
            client_secret: None,
            token_endpoint: "https://auth.test/token".into(),
            resource: "https://mcp.test/mcp".into(),
        };
        let refreshed = kept.answered(&json!({ "access_token": "new", "expires_in": 3600 }), 1000).expect("tokens");
        assert_eq!((refreshed.access_token.as_str(), refreshed.refresh_token.as_deref()), ("new", Some("r1")));
        assert!(refreshed.fresh(4000) && !refreshed.fresh(4550));
        assert!(kept.fresh(i64::MAX - 100), "no expiry said: good until refused");
        let refused = kept.answered(&json!({ "error": "invalid_grant" }), 0).expect_err("refused");
        assert_eq!(refused, "invalid_grant");
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    #[test]
    fn only_https_or_this_computer_is_used_for_signing_in() {
        assert!(endpoint_ok("https://auth.test/authorize"));
        assert!(endpoint_ok("http://127.0.0.1:8080/token"));
        assert!(endpoint_ok("http://localhost/mcp"));
        for refused in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "http://auth.test/token",
            "https://10.0.0.5/token",
            "https://169.254.169.254/x",
            "myapp://open",
        ] {
            assert!(!endpoint_ok(refused), "{refused}");
        }
    }

    #[test]
    fn the_tokens_never_print() {
        let tokens = McpTokens {
            access_token: "secret-a".into(),
            refresh_token: Some("secret-r".into()),
            expires_at: None,
            client_id: "c".into(),
            client_secret: Some("secret-c".into()),
            token_endpoint: "https://auth.test/token".into(),
            resource: "https://mcp.test".into(),
        };
        assert!(!format!("{tokens:?}").contains("secret"));
    }
}
