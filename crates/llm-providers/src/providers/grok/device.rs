use llm_providers::ProviderError;
use llm_providers::oauth::form;
use serde_json::Value;

pub const CLIENT: &str = "b1a00492-073a-47ea-816f-4c329264a828";
pub const SCOPE: &str = "openid profile email offline_access grok-cli:access api:access";
pub const CODE: &str = "https://auth.x.ai/oauth2/device/code";
pub const TOKEN: &str = "https://auth.x.ai/oauth2/token";
pub const GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const MAX_INTERVAL: u64 = 60;

pub fn code_body() -> String {
    form(&[("client_id", CLIENT), ("scope", SCOPE)])
}

pub fn poll_body(device_code: &str) -> String {
    form(&[("grant_type", GRANT), ("client_id", CLIENT), ("device_code", device_code)])
}

pub fn headers() -> Vec<(String, String)> {
    vec![
        ("content-type".into(), "application/x-www-form-urlencoded".into()),
        ("accept".into(), "application/json".into()),
    ]
}

#[derive(Clone, PartialEq, Eq)]
pub struct Device {
    pub device_code: String,
    pub user_code: String,
    pub open: String,
    pub interval: u64,
    pub expires_in: u64,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Device").field("open", &self.open).finish_non_exhaustive()
    }
}

pub fn device(status: u16, body: &str) -> Result<Device, ProviderError> {
    let failed = || ProviderError::Unauthorized(format!("xAI refused the device sign-in (HTTP {status})"));
    if status != 200 {
        return Err(failed());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| failed())?;
    let text = |key: &str| value[key].as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let open = text("verification_uri_complete").or_else(|| text("verification_uri")).ok_or_else(failed)?;
    if !open.starts_with("https://") {
        return Err(failed());
    }
    Ok(Device {
        device_code: text("device_code").ok_or_else(failed)?,
        user_code: text("user_code").ok_or_else(failed)?,
        open,
        interval: value["interval"].as_u64().unwrap_or(5).max(1),
        expires_in: value["expires_in"].as_u64().unwrap_or(900).max(60),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Poll {
    Pending,
    SlowDown,
    Granted,
}

pub fn poll(status: u16, body: &str) -> Result<Poll, ProviderError> {
    let value: Value = serde_json::from_str(body).unwrap_or_default();
    if status == 200 && value["access_token"].as_str().is_some_and(|s| !s.is_empty()) {
        return Ok(Poll::Granted);
    }
    match value["error"].as_str() {
        Some("authorization_pending") => Ok(Poll::Pending),
        Some("slow_down") => Ok(Poll::SlowDown),
        Some(error) => Err(ProviderError::Unauthorized(
            value["error_description"].as_str().filter(|s| !s.is_empty()).unwrap_or(error).to_owned(),
        )),
        None => Err(ProviderError::Unauthorized(format!("xAI sign-in failed (HTTP {status})"))),
    }
}

pub fn slower(wait: u64) -> u64 {
    (wait + 5).min(MAX_INTERVAL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_is_asked_for_with_the_grok_scopes() {
        assert!(code_body().starts_with("client_id=b1a00492-073a-47ea-816f-4c329264a828&scope="));
        assert!(code_body().contains("api%3Aaccess"));
        assert!(poll_body("dc").starts_with("grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code"));
    }

    #[test]
    fn the_complete_link_is_opened_when_there_is_one() {
        let body = r#"{"device_code":"dc","user_code":"AB-12","verification_uri":"https://accounts.x.ai/device",
            "verification_uri_complete":"https://accounts.x.ai/device?code=AB-12","interval":5,"expires_in":600}"#;
        let found = device(200, body).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(found.open, "https://accounts.x.ai/device?code=AB-12");
        assert_eq!((found.user_code.as_str(), found.interval, found.expires_in), ("AB-12", 5, 600));
        assert!(device(400, body).is_err());
        assert!(device(200, r#"{"device_code":"d","user_code":"u","verification_uri":"http://x"}"#).is_err());
    }

    #[test]
    fn polling_waits_slows_down_or_is_granted() {
        assert_eq!(poll(400, r#"{"error":"authorization_pending"}"#), Ok(Poll::Pending));
        assert_eq!(poll(400, r#"{"error":"slow_down"}"#), Ok(Poll::SlowDown));
        assert_eq!(poll(200, r#"{"access_token":"a","refresh_token":"r"}"#), Ok(Poll::Granted));
        assert_eq!(
            poll(400, r#"{"error":"access_denied","error_description":"denied"}"#),
            Err(ProviderError::Unauthorized("denied".into()))
        );
        assert_eq!((slower(5), slower(58)), (10, 60));
    }
}
