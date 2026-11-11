use super::idc::{self, IdcClient, PROFILE_REGIONS, PortalAnswer};
use crate::oauth::{Tokens, challenge};
use crate::ProviderError;
use crate::sign_in::{Host, post};

const PORTAL_CALLBACK: &str = "/signin/callback";

pub fn sign_in(host: &dyn Host) -> Result<Tokens, ProviderError> {
    let listener = host.listen(super::auth::CALLBACK.0, super::auth::CALLBACK.1)?;
    let redirect = listener.redirect_uri();
    let (verifier, state) = (host.random(64)?, host.random(32)?);
    host.open(super::auth::portal(&challenge(&verifier), &state, &redirect), None);
    let answer = listener.next(&state, &[PORTAL_CALLBACK])?;
    match idc::portal_answer(answer.path(), answer.query()) {
        Err(error) => {
            answer.finish(Err(error.to_string()));
            Err(error)
        }
        Ok(PortalAnswer::Social) => {
            let query = answer.query().to_owned();
            let code = answer.code();
            answer.finish(code.as_ref().map(|_| ()).map_err(ToString::to_string));
            let redirect = super::auth::redirect(&redirect, &query)?;
            let (status, body) = post(host, &super::auth::exchange(&code?, &verifier, &redirect))?;
            super::auth::tokens(status, &body, None, host.now())
        }
        Ok(PortalAnswer::Organization { start_url, region }) => {
            let redirect = format!("http://127.0.0.1:{}/oauth/callback", listener.port());
            let client = match register(host, &region, &start_url, &redirect) {
                Ok(client) => client,
                Err(error) => {
                    answer.finish(Err(error.to_string()));
                    return Err(error);
                }
            };
            let (verifier, state) = (host.random(64)?, host.random(32)?);
            answer.redirect(&idc::authorize(&client, &redirect, &state, &challenge(&verifier)));
            let back = listener.next(&state, &[])?;
            let tokens = back.code().and_then(|code| organization(host, &client, &code, &verifier, &redirect));
            back.finish(tokens.as_ref().map(|_| ()).map_err(ToString::to_string));
            tokens
        }
    }
}

fn register(host: &dyn Host, region: &str, start_url: &str, redirect: &str) -> Result<IdcClient, ProviderError> {
    let (status, body) = post(host, &idc::register(region, start_url, redirect))?;
    idc::registered(region, status, &body)
}

fn organization(
    host: &dyn Host,
    client: &IdcClient,
    code: &str,
    verifier: &str,
    redirect: &str,
) -> Result<Tokens, ProviderError> {
    let (status, body) = post(host, &idc::exchange(client, code, verifier, redirect))?;
    let mut tokens = idc::tokens(client, status, &body, None, host.now())?;
    tokens.account_id = profile(host, &tokens.access_token)?;
    Ok(tokens)
}

fn profile(host: &dyn Host, access_token: &str) -> Result<String, ProviderError> {
    for region in PROFILE_REGIONS {
        let (url, headers, body) = idc::profiles(region, access_token);
        let Ok((status, text)) = host.post(&url, &headers, &body) else { continue };
        if let Some(arn) = idc::first_profile(status, &text) {
            return Ok(arn);
        }
    }
    Err(ProviderError::Unauthorized(
        "signed in, but this account has no Kiro profile; ask your organization's administrator to add you to one"
            .into(),
    ))
}

#[cfg(test)]
#[path = "sign_in_tests.rs"]
mod tests;
