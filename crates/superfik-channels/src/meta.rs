//! What Meta's APIs (WhatsApp Cloud, Instagram) share: the Graph API's
//! address, the webhook's handshake and signature, and fetching a file a
//! message names.

use superfik_core::channel::MAX_MEDIA_BYTES;

use crate::http::{Http, HttpRequest};
use crate::{HookAnswer, HookMethod, HookRequest, signature};

pub const GRAPH_VERSION: &str = "v21.0";

pub fn hook(app_secret: &str, verify_token: &str, request: &HookRequest, take: impl FnOnce(&[u8])) -> HookAnswer {
    match request.method {
        HookMethod::Get => signature::meta_challenge(verify_token, request),
        HookMethod::Post if signature::meta(app_secret, request) => {
            take(&request.body);
            HookAnswer::Accepted
        }
        HookMethod::Post => HookAnswer::Refused,
    }
}

const CDN_DOMAINS: [&str; 2] = ["fbcdn.net", "cdninstagram.com"];
const CDN_HOSTS: [&str; 2] = ["lookaside.fbsbx.com", "lookaside.facebook.com"];

pub fn is_meta_cdn(url: &str) -> bool {
    let Some(scheme) = url.get(..8).filter(|s| s.eq_ignore_ascii_case("https://")) else { return false };
    let rest = &url[scheme.len()..];
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains(['@', '\\']) {
        return false;
    }
    let host = match authority.rsplit_once(':') {
        Some((host, "443")) => host,
        Some(_) => return false,
        None => authority,
    };
    let host = host.to_ascii_lowercase();
    let label = |l: &str| !l.is_empty() && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    if !host.split('.').all(label) {
        return false;
    }
    CDN_HOSTS.contains(&host.as_str())
        || CDN_DOMAINS.iter().any(|domain| host.strip_suffix(domain).is_some_and(|sub| sub.ends_with('.')))
}

pub fn download(http: &dyn Http, url: &str, token: Option<&str>) -> Option<Vec<u8>> {
    if !is_meta_cdn(url) {
        tracing::warn!("a media address off Meta's CDN was not fetched");
        return None;
    }
    let request = HttpRequest::get(url).no_redirects().at_most(MAX_MEDIA_BYTES + 1);
    let request = match token {
        Some(token) => request.bearer(token),
        None => request,
    };
    let response = http.call(request).ok().filter(|r| r.is_success())?;
    (u64::try_from(response.body.len()).ok()? <= MAX_MEDIA_BYTES).then_some(response.body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::{HttpResponse, ScriptedHttp};

    #[test]
    fn only_metas_cdn_over_https_is_a_media_address() {
        for good in [
            "https://lookaside.fbsbx.com/ig_messaging_cdn/?asset_id=1",
            "https://scontent-gru1-1.cdninstagram.com/v/t51/a.jpg?x=1",
            "https://scontent.xx.fbcdn.net/v/a.png",
            "HTTPS://Lookaside.FBSBX.com/whatsapp_business/attachments/?mid=1",
            "https://lookaside.fbsbx.com:443/a",
        ] {
            assert!(is_meta_cdn(good), "{good}");
        }
        for bad in [
            "http://lookaside.fbsbx.com/a",
            "https://169.254.169.254/latest/meta-data",
            "https://localhost/a",
            "https://fbcdn.net.evil.com/a",
            "https://evilfbcdn.net/a",
            "https://lookaside.fbsbx.com@evil.com/a",
            "https://evil.com\\@lookaside.fbsbx.com/a",
            "https://lookaside.fbsbx.com:8080/a",
            "file:///etc/passwd",
            "https://",
        ] {
            assert!(!is_meta_cdn(bad), "{bad}");
        }
    }

    #[test]
    fn a_download_off_metas_cdn_is_never_asked() {
        let http = ScriptedHttp::new([HttpResponse { status: 200, body: b"x".to_vec() }]);
        assert_eq!(download(&http, "https://169.254.169.254/latest", Some("token")), None);
        assert!(http.calls().is_empty(), "the token never leaves for another host");
    }

    #[test]
    fn a_download_follows_no_redirect_and_reads_no_more_than_a_file_may_hold() {
        let http = ScriptedHttp::new([HttpResponse { status: 302, body: Vec::new() }]);
        assert_eq!(download(&http, "https://lookaside.fbsbx.com/a", None), None);
        let call = &http.calls()[0];
        assert!(!call.redirects, "a redirect could lead anywhere");
        assert_eq!(call.limit, Some(MAX_MEDIA_BYTES + 1));
    }
}
