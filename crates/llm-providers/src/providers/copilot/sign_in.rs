use std::time::Duration;

use super::device::{self, Pacing, Poll, Post};
use llm_providers::{Config, Host, ProviderError, Tokens};

pub fn sign_in(host: &dyn Host) -> Result<Config, ProviderError> {
    let (status, body) = post(host, &device::code_request())?;
    let code = device::device(status, &body)?;
    host.open(code.verification_uri.clone(), Some(code.user_code.clone()));
    let token = wait(host, &code)?;
    let (status, body) = host.get(device::USER, &device::user_headers(&token))?;
    let base_url = device::api_base(status, &body)?;
    let config = Config { auth: Some(Tokens::bearer(&token)), base_url, ..Config::default() };
    config.validate(&super::Provider)?;
    Ok(config)
}

fn wait(host: &dyn Host, code: &device::Device) -> Result<String, ProviderError> {
    let mut pacing = Pacing::new(code.interval);
    let mut waited = 0;
    loop {
        if waited >= code.expires_in {
            return Err(ProviderError::Unauthorized("the GitHub code expired before it was approved".into()));
        }
        host.sleep(Duration::from_secs(pacing.wait));
        waited += pacing.wait;
        let Ok((status, body)) = post(host, &device::poll_request(&code.device_code)) else { continue };
        match device::poll(status, &body)? {
            Poll::Granted(token) => return Ok(token),
            Poll::Pending => {}
            Poll::SlowDown(interval) => pacing.slow_down(interval),
        }
    }
}

fn post(host: &dyn Host, request: &Post) -> Result<(u16, String), ProviderError> {
    host.post(&request.url, &device::post_headers(), &request.body)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use llm_providers::testing::ScriptedHost;

    #[test]
    fn the_device_flow_waits_for_approval_and_finds_the_account_api() {
        let host = ScriptedHost::new(
            [
                (
                    200,
                    json!({"device_code": "dc", "user_code": "AB12-CD34", "verification_uri": "https://github.com/login/device",
                        "interval": 5, "expires_in": 900})
                    .to_string(),
                ),
                (200, json!({"error": "authorization_pending"}).to_string()),
                (200, json!({"error": "slow_down", "interval": 10}).to_string()),
                (200, json!({"access_token": "gho_new", "token_type": "bearer"}).to_string()),
                (200, json!({"login": "u", "endpoints": {"api": "https://api.individual.githubcopilot.com"}}).to_string()),
            ],
            [],
        );
        let config = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
        let log = host.log.borrow();
        assert_eq!(log.opened, [("https://github.com/login/device".to_owned(), Some("AB12-CD34".to_owned()))]);
        assert_eq!(log.waits.iter().map(Duration::as_secs).collect::<Vec<_>>(), [6, 6, 15]);
        assert_eq!(config.access_token(), "gho_new");
        assert_eq!(config.base(&super::super::Provider), "https://api.individual.githubcopilot.com");
        assert_eq!(log.sent[0].1, "client_id=Ov23ctDVkRmgkPke0Mmm&scope=read%3Auser");
        assert_eq!(log.sent[4].0, "https://api.github.com/copilot_internal/user");
    }

    #[test]
    fn a_refused_or_expired_code_ends_the_sign_in() {
        let device = json!({"device_code": "dc", "user_code": "C", "verification_uri": "https://github.com/login/device",
            "interval": 5, "expires_in": 10});
        let denied = ScriptedHost::new(
            [
                (200, device.to_string()),
                (200, json!({"error": "access_denied", "error_description": "denied"}).to_string()),
            ],
            [],
        );
        let error = sign_in(&denied).map(|_| ()).err().unwrap_or_else(|| panic!("denied"));
        assert!(error.to_string().contains("denied"));
        let pending = json!({"error": "authorization_pending"}).to_string();
        let slow = ScriptedHost::new([(200, device.to_string()), (200, pending.clone()), (200, pending)], []);
        let error = sign_in(&slow).map(|_| ()).err().unwrap_or_else(|| panic!("expired"));
        assert!(error.to_string().contains("expired"));
    }
}
