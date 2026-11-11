//! The subscriptions' token calls over HTTP; what they send and read is in
//! `llm_providers::oauth`.
use super::super::{ModelError, http::Http};
use domain::ModelProvider;
use llm_providers::oauth::{self as plugin, TOKEN_LIMIT, TokenRequest};

pub use plugin::{
    CLAUDE_CLIENT, CLAUDE_TOKEN, OPENAI_CLIENT, OPENAI_REDIRECT, OPENAI_TOKEN, Tokens, account_id, authorize, form,
};

pub fn exchange(
    http: &dyn Http,
    provider: ModelProvider,
    code: &str,
    state: &str,
    verifier: &str,
    redirect: &str,
) -> Result<Tokens, ModelError> {
    post(http, provider, plugin::exchange(provider, code, state, verifier, redirect), None)
}

pub fn refresh(http: &dyn Http, provider: ModelProvider, previous: &Tokens) -> Result<Tokens, ModelError> {
    post(http, provider, plugin::refresh(provider, previous), Some(previous))
}

fn post(
    http: &dyn Http,
    provider: ModelProvider,
    request: TokenRequest,
    previous: Option<&Tokens>,
) -> Result<Tokens, ModelError> {
    let response = http
        .post(&request.url, &[("content-type".into(), request.content_type.into())], &request.body)
        .map_err(ModelError::Network)?;
    let status = response.status;
    let body = if status == 200 { super::transport::read(response, TOKEN_LIMIT)? } else { String::new() };
    Ok(plugin::tokens(provider, status, &body, previous, chrono::Utc::now())?)
}
