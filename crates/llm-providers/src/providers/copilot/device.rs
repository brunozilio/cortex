use llm_providers::ProviderError;
use llm_providers::oauth::form;
use serde_json::Value;

pub const CLIENT: &str = "Ov23ctDVkRmgkPke0Mmm";
pub const GITHUB: &str = "https://github.com";
pub const USER: &str = "https://api.github.com/copilot_internal/user";
pub const SCOPE: &str = "read:user";
pub const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const MAX_INTERVAL: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    pub url: String,
    pub body: String,
}

pub fn code_request() -> Post {
    Post { url: format!("{GITHUB}/login/device/code"), body: form(&[("client_id", CLIENT), ("scope", SCOPE)]) }
}

pub fn poll_request(device_code: &str) -> Post {
    Post {
        url: format!("{GITHUB}/login/oauth/access_token"),
        body: form(&[("client_id", CLIENT), ("device_code", device_code), ("grant_type", GRANT)]),
    }
}

pub fn post_headers() -> Vec<(String, String)> {
    vec![
        ("content-type".into(), "application/x-www-form-urlencoded".into()),
        ("accept".into(), "application/json".into()),
    ]
}

#[derive(Clone, PartialEq, Eq)]
pub struct Device {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device").field("verification_uri", &self.verification_uri).finish_non_exhaustive()
    }
}

pub fn device(status: u16, body: &str) -> Result<Device, ProviderError> {
    let failed = || ProviderError::Unauthorized(format!("GitHub refused the device sign-in (HTTP {status})"));
    if status != 200 {
        return Err(failed());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| failed())?;
    let text = |key: &str| value[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned).ok_or_else(failed);
    Ok(Device {
        device_code: text("device_code")?,
        user_code: text("user_code")?,
        verification_uri: text("verification_uri")?,
        interval: value["interval"].as_u64().unwrap_or(5).max(1),
        expires_in: value["expires_in"].as_u64().unwrap_or(900),
    })
}

#[derive(Clone, PartialEq, Eq)]
pub enum Poll {
    Pending,
    SlowDown(Option<u64>),
    Granted(String),
}

impl std::fmt::Debug for Poll {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => f.write_str("Pending"),
            Self::SlowDown(interval) => f.debug_tuple("SlowDown").field(interval).finish(),
            Self::Granted(_) => f.write_str("Granted(..)"),
        }
    }
}

pub fn poll(status: u16, body: &str) -> Result<Poll, ProviderError> {
    let value: Value = serde_json::from_str(body).unwrap_or_default();
    if let Some(token) = value["access_token"].as_str().filter(|s| !s.is_empty()) {
        return Ok(Poll::Granted(token.to_owned()));
    }
    match value["error"].as_str() {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown(value["interval"].as_u64())),
        Some(error) => Err(ProviderError::Unauthorized(
            value["error_description"].as_str().filter(|s| !s.is_empty()).unwrap_or(error).to_owned(),
        )),
        None => Err(ProviderError::Unauthorized(format!("GitHub sign-in failed (HTTP {status})"))),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pacing {
    interval: u64,
    slowed: u64,
    pub wait: u64,
}

impl Pacing {
    pub fn new(interval: u64) -> Self {
        Self { interval, slowed: 0, wait: (interval * 6).div_ceil(5) }
    }

    pub fn slow_down(&mut self, interval: Option<u64>) {
        self.slowed += 1;
        let base = interval.unwrap_or(self.interval);
        self.wait = (base + 5 * self.slowed).min(MAX_INTERVAL);
    }
}

pub fn api_base(status: u16, body: &str) -> Result<String, ProviderError> {
    match status {
        200 => {}
        401 | 403 | 404 => {
            return Err(ProviderError::Unauthorized("this GitHub account has no Copilot subscription".to_owned()));
        }
        _ => return Err(ProviderError::Unauthorized(format!("GitHub returned HTTP {status} for the Copilot user"))),
    }
    let value: Value =
        serde_json::from_str(body).map_err(|_| ProviderError::Unauthorized("invalid Copilot user response".into()))?;
    Ok(value["endpoints"]["api"]
        .as_str()
        .map(|api| api.trim_end_matches('/'))
        .filter(|api| api.starts_with("https://"))
        .unwrap_or(super::API)
        .to_owned())
}

pub fn user_headers(token: &str) -> Vec<(String, String)> {
    vec![
        ("authorization".into(), format!("Bearer {token}")),
        ("accept".into(), "application/vnd.github+json".into()),
        ("x-github-api-version".into(), "2022-11-28".into()),
        ("user-agent".into(), super::user_agent()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_device_code_is_asked_for_with_the_copilot_cli_client() {
        let request = code_request();
        assert_eq!(request.url, "https://github.com/login/device/code");
        assert_eq!(request.body, "client_id=Ov23ctDVkRmgkPke0Mmm&scope=read%3Auser");
        let polled = poll_request("dc");
        assert_eq!(polled.url, "https://github.com/login/oauth/access_token");
        assert!(polled.body.ends_with("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    }

    #[test]
    fn a_device_answer_gives_the_code_to_type() {
        let body = r#"{"device_code":"dc","user_code":"AB12-CD34","verification_uri":"https://github.com/login/device","interval":5,"expires_in":899}"#;
        let found = device(200, body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((found.user_code.as_str(), found.interval, found.expires_in), ("AB12-CD34", 5, 899));
        assert!(device(400, body).is_err());
        assert!(device(200, "{}").is_err());
    }

    #[test]
    fn polling_waits_slows_down_or_gets_the_token() {
        assert_eq!(poll(200, r#"{"error":"authorization_pending"}"#), Ok(Poll::Pending));
        assert_eq!(poll(200, r#"{"error":"slow_down","interval":10}"#), Ok(Poll::SlowDown(Some(10))));
        assert_eq!(poll(200, r#"{"access_token":"gho_1","token_type":"bearer"}"#), Ok(Poll::Granted("gho_1".into())));
        assert_eq!(
            poll(200, r#"{"error":"access_denied","error_description":"The user said no"}"#),
            Err(ProviderError::Unauthorized("The user said no".into()))
        );
        assert!(poll(500, "").is_err());
    }

    #[test]
    fn pacing_follows_the_copilot_cli() {
        let mut pacing = Pacing::new(5);
        assert_eq!(pacing.wait, 6);
        pacing.slow_down(Some(10));
        assert_eq!(pacing.wait, 15);
        pacing.slow_down(None);
        assert_eq!(pacing.wait, 15);
        pacing.slow_down(Some(58));
        assert_eq!(pacing.wait, 60);
    }

    #[test]
    fn the_api_base_comes_from_the_copilot_user() {
        let business = r#"{"login":"u","endpoints":{"api":"https://api.business.githubcopilot.com/"}}"#;
        assert_eq!(api_base(200, business), Ok("https://api.business.githubcopilot.com".into()));
        assert_eq!(api_base(200, r#"{"login":"u"}"#), Ok("https://api.githubcopilot.com".into()));
        assert_eq!(api_base(200, r#"{"endpoints":{"api":"http://evil"}}"#), Ok("https://api.githubcopilot.com".into()));
        assert!(matches!(api_base(404, ""), Err(ProviderError::Unauthorized(_))));
    }
}
