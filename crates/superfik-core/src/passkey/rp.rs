//! Whether a relying party id belongs to the page that names it.

use url::Url;

pub fn rp_id_allowed(rp_id: &str, origin: &str) -> bool {
    let Ok(origin) = Url::parse(origin) else { return false };
    let Some(host) = origin.host_str().map(str::to_ascii_lowercase) else { return false };
    let rp_id = rp_id.to_ascii_lowercase();
    let local = host == "localhost";
    let secure = origin.scheme() == "https" || (local && origin.scheme() == "http");
    let within = host == rp_id || host.ends_with(&format!(".{rp_id}"));
    let registrable = local && rp_id == "localhost" || psl::domain_str(&rp_id).is_some();
    secure && within && registrable
}
