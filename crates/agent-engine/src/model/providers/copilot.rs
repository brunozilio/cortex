use std::time::Duration;

use super::super::{ModelError, http::Http};
use super::Config;
use llm_providers::copilot::device::{self, Pacing, Poll, Post};
use llm_providers::oauth::{TOKEN_LIMIT, Tokens};

pub fn sign_in(
    http: &dyn Http,
    show: &mut dyn FnMut(String, String),
    sleep: &dyn Fn(Duration),
) -> Result<Config, ModelError> {
    let (status, body) = post(http, &device::code_request())?;
    let code = device::device(status, &body)?;
    show(code.verification_uri.clone(), code.user_code.clone());
    let token = wait(http, &code, sleep)?;
    let response = http.get(device::USER, &device::user_headers(&token)).map_err(ModelError::Network)?;
    let status = response.status;
    let body = super::transport::read(response, TOKEN_LIMIT)?;
    let base_url = device::api_base(status, &body)?;
    let config = Config { auth: Some(Tokens::bearer(&token)), base_url, ..Config::default() };
    config.validate(domain::ModelProvider::CopilotSubscription)?;
    Ok(config)
}

fn wait(http: &dyn Http, code: &device::Device, sleep: &dyn Fn(Duration)) -> Result<String, ModelError> {
    let mut pacing = Pacing::new(code.interval);
    let mut waited = 0;
    loop {
        if waited >= code.expires_in {
            return Err(ModelError::Unauthorized("the GitHub code expired before it was approved".into()));
        }
        sleep(Duration::from_secs(pacing.wait));
        waited += pacing.wait;
        let (status, body) = match post(http, &device::poll_request(&code.device_code)) {
            Ok(answer) => answer,
            Err(error) => {
                tracing::warn!(%error, "GitHub sign-in poll failed, retrying");
                continue;
            }
        };
        match device::poll(status, &body)? {
            Poll::Granted(token) => return Ok(token),
            Poll::Pending => {}
            Poll::SlowDown(interval) => pacing.slow_down(interval),
        }
    }
}

fn post(http: &dyn Http, request: &Post) -> Result<(u16, String), ModelError> {
    let response = http.post(&request.url, &device::post_headers(), &request.body).map_err(ModelError::Network)?;
    let status = response.status;
    Ok((status, super::transport::read(response, TOKEN_LIMIT)?))
}
