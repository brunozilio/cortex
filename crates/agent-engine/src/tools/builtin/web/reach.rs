use domain::permission_rules::{is_private, site};

use crate::model::CancellationToken;
use crate::model::http::{Http, Response, reason};
use crate::model::web_client::private_among;

pub const MOST_REDIRECTS: usize = 5;
const REDIRECTS: [u16; 5] = [301, 302, 303, 307, 308];

#[derive(Debug)]
pub struct Reached {
    pub url: String,
    pub response: Response,
}

pub fn secure(url: &str) -> String {
    url.replacen("http://", "https://", 1)
}

fn private(why: &str) -> String {
    format!("{why}; only the public web is read.")
}

fn public_host(url: &str) -> Result<String, String> {
    let host = site(url).filter(|_| url.starts_with("https://")).ok_or(format!("Not a web address: {url}"))?;
    if is_private(&host) {
        return Err(private(&format!("{host} is this computer or its private network")));
    }
    Ok(host)
}

pub fn refusal_by_name(url: &str) -> Option<String> {
    public_host(url).err()
}

pub fn refusal(url: &str, web: &dyn Http) -> Option<String> {
    let host = match public_host(url) {
        Ok(host) => host,
        Err(why) => return Some(why),
    };
    match web.addresses(&host) {
        Ok(addresses) => private_among(&host, &addresses).map(|why| private(&why)),
        Err(error) => Some(format!("Could not fetch {url}: {error}")),
    }
}

pub async fn fetch(
    web: &dyn Http,
    url: &str,
    headers: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<Reached, String> {
    let mut url = secure(url);
    for _ in 0..=MOST_REDIRECTS {
        if let Some(why) = refusal(&url, web) {
            return Err(why);
        }
        let response = web
            .one_hop(&url, headers, None, cancel)
            .await
            .map_err(|error| format!("Could not fetch {url}: {}", reason(error)))?;
        match redirected(&url, &response) {
            Some(next) => url = secure(&next),
            None => return Ok(Reached { url, response }),
        }
    }
    Err(format!("{url} is one redirect too many: {MOST_REDIRECTS} are followed at most."))
}

fn redirected(from: &str, response: &Response) -> Option<String> {
    let location = response.header("location").filter(|_| REDIRECTS.contains(&response.status))?;
    let joined = url::Url::parse(from).ok().and_then(|base| base.join(&location).ok());
    Some(joined.map_or(location, String::from))
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod tests;
