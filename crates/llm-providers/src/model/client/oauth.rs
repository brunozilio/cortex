use super::super::{CancellationToken, ModelError, http::Http};
use llm_providers::Provider;
use llm_providers::oauth::{TOKEN_LIMIT, Tokens};

pub async fn refresh(
    http: &dyn Http,
    provider: &dyn Provider,
    previous: &Tokens,
    cancel: &CancellationToken,
) -> Result<Tokens, ModelError> {
    let request = provider
        .refresh(previous)
        .ok_or_else(|| ModelError::Unauthorized(format!("{} keeps no refreshable tokens", provider.name())))?;
    let headers = [("content-type".to_owned(), request.content_type.to_owned())];
    let response = http.post_cancellable(&request.url, &headers, &request.body, cancel).await?;
    let status = response.status;
    let body = if status == 200 {
        response.body.text(cancel, usize::try_from(TOKEN_LIMIT).unwrap_or(usize::MAX)).await?
    } else {
        String::new()
    };
    Ok(provider.tokens(status, &body, Some(previous), jiff::Timestamp::now())?)
}
