//! Kiro's browser sign-in, as Kiro CLI does it. Its portal answers on the
//! loopback in one of two ways: Google and GitHub come back to
//! `/oauth/callback` with a code for Kiro's auth service; an organization
//! (AWS IAM Identity Center) comes back to `/signin/callback` naming its
//! start URL and region, and the same browser tab is sent on to that
//! organization's sign-in, which returns its code to `/oauth/callback`.

use superfik_core::ModelProvider;
use superfik_native::model::http::Http;
use superfik_native::model::providers::{Tokens, kiro_idc, oauth};
use superfik_providers::kiro::idc::{self, PortalAnswer};

use super::redirect::{Loopback, pkce};

/// Where the portal returns an organization sign-in.
const PORTAL_CALLBACK: &str = "/signin/callback";
const KIRO: ModelProvider = ModelProvider::KiroSubscription;

/// Signs in on `listener` (Kiro's loopback), `open` showing the portal.
pub(super) fn sign_in(
    http: &dyn Http,
    listener: &Loopback,
    open: &mut dyn FnMut(String, Option<String>),
    random: &dyn Fn(usize) -> Result<String, String>,
) -> Result<Tokens, String> {
    let redirect = listener.redirect_uri();
    let (verifier, state) = (random(64)?, random(32)?);
    open(oauth::authorize(KIRO, &pkce::challenge(&verifier), &state, &redirect), None);
    let answer = listener.next(&state, &[PORTAL_CALLBACK])?;
    let picked = idc::portal_answer(&answer.path, &answer.query).map_err(|error| error.to_string());
    match picked {
        Err(error) => {
            answer.finish(Err(&error));
            Err(error)
        }
        Ok(PortalAnswer::Social) => {
            let query = answer.query.clone();
            let code = answer.code();
            answer.finish(code.as_ref().map(|_| ()));
            let redirect = superfik_providers::oauth::exchange_redirect(KIRO, &redirect, &query)
                .map_err(|error| error.to_string())?;
            oauth::exchange(http, KIRO, &code?, &state, &verifier, &redirect).map_err(|error| error.to_string())
        }
        Ok(PortalAnswer::Organization { start_url, region }) => {
            let redirect = format!("http://127.0.0.1:{}/oauth/callback", listener.port());
            let client = match kiro_idc::register(http, &region, &start_url, &redirect) {
                Ok(client) => client,
                Err(error) => {
                    let error = error.to_string();
                    answer.finish(Err(&error));
                    return Err(error);
                }
            };
            let (verifier, state) = (random(64)?, random(32)?);
            answer.redirect(&idc::authorize(&client, &redirect, &state, &pkce::challenge(&verifier)));
            let back = listener.next(&state, &[])?;
            let tokens = back.code().and_then(|code| {
                kiro_idc::sign_in(http, &client, &code, &verifier, &redirect).map_err(|error| error.to_string())
            });
            back.finish(tokens.as_ref().map(|_| ()));
            tokens
        }
    }
}

#[cfg(test)]
#[path = "kiro_sign_in_tests.rs"]
mod tests;
