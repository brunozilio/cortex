//! Pure parts of the MCP authorization flow: well-known URLs (RFC 9728,
//! RFC 8414), metadata, the authorization URL and token responses.

use serde::Deserialize;
use serde_json::Value;

pub fn split_origin(url: &str) -> Option<(&str, &str)> {
    let after_scheme = url.find("://")? + 3;
    let rest = &url[after_scheme..];
    let path_start = rest.find(['/', '?', '#']).map_or(url.len(), |i| after_scheme + i);
    let path = url[path_start..].split(['?', '#']).next().unwrap_or_default();
    Some((&url[..path_start], path.trim_end_matches('/')))
}

pub fn protected_resource_url(resource: &str) -> Option<String> {
    let (origin, path) = split_origin(resource)?;
    Some(format!("{origin}/.well-known/oauth-protected-resource{path}"))
}

pub fn authorization_server_urls(issuer: &str) -> Vec<String> {
    let Some((origin, path)) = split_origin(issuer) else { return Vec::new() };
    let mut urls = vec![
        format!("{origin}/.well-known/oauth-authorization-server{path}"),
        format!("{origin}/.well-known/openid-configuration{path}"),
    ];
    if !path.is_empty() {
        urls.push(format!("{origin}{path}/.well-known/openid-configuration"));
    }
    urls
}

pub fn authorization_server(resource_metadata: &Value) -> Option<String> {
    resource_metadata["authorization_servers"].as_array()?.first()?.as_str().map(str::to_owned)
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerMetadata {
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub registration_endpoint: Option<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authorize<'a> {
    pub endpoint: &'a str,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub challenge: &'a str,
    pub state: &'a str,
    pub resource: &'a str,
    pub scopes: &'a [String],
}

pub fn authorize_url(a: &Authorize) -> String {
    let mut params = vec![
        ("response_type", "code".to_owned()),
        ("client_id", a.client_id.to_owned()),
        ("redirect_uri", a.redirect_uri.to_owned()),
        ("code_challenge", a.challenge.to_owned()),
        ("code_challenge_method", "S256".to_owned()),
        ("state", a.state.to_owned()),
        ("resource", a.resource.to_owned()),
    ];
    if !a.scopes.is_empty() {
        params.push(("scope", a.scopes.join(" ")));
    }
    let separator = if a.endpoint.contains('?') { '&' } else { '?' };
    format!("{}{separator}{}", a.endpoint, form(&params))
}

pub fn form(params: &[(&str, String)]) -> String {
    params.iter().map(|(key, value)| format!("{}={}", encode(key), encode(value))).collect::<Vec<_>>().join("&")
}

pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_in: Option<u64>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn well_known_urls_insert_before_the_path() {
        assert_eq!(split_origin("https://mcp.linear.app/mcp?x=1"), Some(("https://mcp.linear.app", "/mcp")));
        assert_eq!(split_origin("https://a.com"), Some(("https://a.com", "")));
        assert_eq!(split_origin("nope"), None);
        assert_eq!(
            protected_resource_url("https://mcp.linear.app/mcp").as_deref(),
            Some("https://mcp.linear.app/.well-known/oauth-protected-resource/mcp")
        );
        assert_eq!(
            authorization_server_urls("https://auth.x.com"),
            vec![
                "https://auth.x.com/.well-known/oauth-authorization-server",
                "https://auth.x.com/.well-known/openid-configuration",
            ]
        );
        assert_eq!(authorization_server_urls("https://x.com/t1").len(), 3);
    }

    #[test]
    fn metadata_is_read() {
        let resource = json!({"resource": "https://m/mcp", "authorization_servers": ["https://auth.m"]});
        assert_eq!(authorization_server(&resource).as_deref(), Some("https://auth.m"));
        assert_eq!(authorization_server(&json!({})), None);
        let server: ServerMetadata = serde_json::from_value(json!({
            "authorization_endpoint": "https://auth.m/authorize",
            "token_endpoint": "https://auth.m/token",
            "registration_endpoint": "https://auth.m/register"
        }))
        .expect("metadata");
        assert_eq!(server.registration_endpoint.as_deref(), Some("https://auth.m/register"));
    }

    #[test]
    fn authorize_url_carries_pkce_state_and_resource() {
        let scopes = vec!["read".to_owned(), "write".to_owned()];
        let url = authorize_url(&Authorize {
            endpoint: "https://auth.m/authorize",
            client_id: "c 1",
            redirect_uri: "http://127.0.0.1:5000/callback",
            challenge: "abc",
            state: "s",
            resource: "https://m/mcp",
            scopes: &scopes,
        });
        assert!(url.starts_with("https://auth.m/authorize?response_type=code&client_id=c%201&"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A5000%2Fcallback"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.ends_with("&scope=read%20write"));
    }
}
