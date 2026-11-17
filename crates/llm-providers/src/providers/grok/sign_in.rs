use std::time::Duration;

use super::device::{self, Poll};
use llm_providers::{Config, Host, Provider as Shape, ProviderError};

pub fn sign_in(host: &dyn Host) -> Result<Config, ProviderError> {
    let (status, body) = host.post(device::CODE, &device::headers(), &device::code_body())?;
    let code = device::device(status, &body)?;
    host.open(code.open.clone(), Some(code.user_code.clone()));
    let (status, body) = wait(host, &code)?;
    let tokens = super::Provider.tokens(status, &body, None, host.now())?;
    let config = Config { auth: Some(tokens), ..Config::default() };
    config.validate(&super::Provider)?;
    Ok(config)
}

fn wait(host: &dyn Host, code: &device::Device) -> Result<(u16, String), ProviderError> {
    let mut pause = code.interval;
    let mut waited = 0;
    loop {
        if waited >= code.expires_in {
            return Err(ProviderError::Unauthorized("the xAI code expired before it was approved".into()));
        }
        host.sleep(Duration::from_secs(pause));
        waited += pause;
        let Ok((status, body)) = host.post(device::TOKEN, &device::headers(), &device::poll_body(&code.device_code))
        else {
            continue;
        };
        match device::poll(status, &body)? {
            Poll::Granted => return Ok((status, body)),
            Poll::Pending => {}
            Poll::SlowDown => pause = device::slower(pause),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use llm_providers::testing::ScriptedHost;

    fn code(expires_in: u64) -> String {
        json!({"device_code": "dc", "user_code": "AB-12", "verification_uri": "https://accounts.x.ai/device",
            "verification_uri_complete": "https://accounts.x.ai/device?code=AB-12", "interval": 5, "expires_in": expires_in})
        .to_string()
    }

    #[test]
    fn the_device_flow_waits_for_approval_and_keeps_both_tokens() {
        let host = ScriptedHost::new(
            [
                (200, code(600)),
                (400, json!({"error": "authorization_pending"}).to_string()),
                (400, json!({"error": "slow_down"}).to_string()),
                (200, json!({"access_token": "at", "refresh_token": "rt", "expires_in": 21600}).to_string()),
            ],
            [],
        );
        let config = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
        let tokens = config.refreshable().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!((tokens.access_token.as_str(), tokens.refresh_token.as_str()), ("at", "rt"));
        let log = host.log.borrow();
        assert_eq!(log.opened, [("https://accounts.x.ai/device?code=AB-12".to_owned(), Some("AB-12".to_owned()))]);
        assert_eq!(log.waits.iter().map(Duration::as_secs).collect::<Vec<_>>(), [5, 5, 10]);
        let urls: Vec<_> = log.sent.iter().map(|(url, _)| url.as_str()).collect();
        assert_eq!(urls, [device::CODE, device::TOKEN, device::TOKEN, device::TOKEN]);
    }

    #[test]
    fn a_refused_or_expired_code_ends_the_sign_in() {
        let denied = ScriptedHost::new(
            [(200, code(600)), (400, json!({"error": "access_denied", "error_description": "denied"}).to_string())],
            [],
        );
        let error = sign_in(&denied).map(|_| ()).err().unwrap_or_else(|| panic!("denied"));
        assert!(error.to_string().contains("denied"));
        let pending = json!({"error": "authorization_pending"}).to_string();
        let mut answers = vec![(200, code(60))];
        answers.extend(std::iter::repeat_n((400, pending), 12));
        let slow = ScriptedHost::new(answers, []);
        let error = sign_in(&slow).map(|_| ()).err().unwrap_or_else(|| panic!("expired"));
        assert!(error.to_string().contains("expired"));
    }
}
