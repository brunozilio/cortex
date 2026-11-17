use super::super::{ModelError, http::Http};
use llm_providers::Provider;
use llm_providers::oauth::{TOKEN_LIMIT, Tokens};

pub fn refresh(http: &dyn Http, provider: &dyn Provider, previous: &Tokens) -> Result<Tokens, ModelError> {
    let request = provider
        .refresh(previous)
        .ok_or_else(|| ModelError::Unauthorized(format!("{} keeps no refreshable tokens", provider.name())))?;
    let response = http
        .post(&request.url, &[("content-type".into(), request.content_type.into())], &request.body)
        .map_err(ModelError::Network)?;
    let status = response.status;
    let body = if status == 200 { super::transport::read(response, TOKEN_LIMIT)? } else { String::new() };
    Ok(provider.tokens(status, &body, Some(previous), chrono::Utc::now())?)
}
