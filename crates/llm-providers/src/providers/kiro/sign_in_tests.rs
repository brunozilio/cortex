use super::*;
use llm_providers::testing::ScriptedHost;

const ARN: &str = "arn:aws:codewhisperer:us-east-1:111122223333:profile/ABCDEF";

#[test]
fn an_organization_sign_in_goes_on_to_its_identity_center_in_the_same_tab() {
    let host = ScriptedHost::new(
        [
            (200, r#"{"clientId":"cid","clientSecret":"secret"}"#.to_owned()),
            (200, r#"{"accessToken":"a","refreshToken":"r","expiresIn":3600}"#.to_owned()),
            (403, String::new()),
            (200, format!(r#"{{"profiles":[{{"arn":"{ARN}"}}]}}"#)),
        ],
        [
            (
                "/signin/callback",
                "login_option=awsidc&issuer_url=https%3A%2F%2Facme.awsapps.com%2Fstart&idc_region=eu-west-1&state=r2x32",
            ),
            ("/oauth/callback", "code=org-code&state=r4x32"),
        ],
    );
    let tokens = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((tokens.access_token.as_str(), tokens.account_id.as_str()), ("a", ARN));
    assert!(idc::client(&tokens).is_some(), "kept for refreshes");
    let log = host.log.borrow();
    assert!(log.opened[0].0.starts_with("https://app.kiro.dev/signin?"), "{:?}", log.opened);
    assert!(
        log.answered[0].starts_with("redirect https://oidc.eu-west-1.amazonaws.com/authorize?"),
        "{:?}",
        log.answered
    );
    assert_eq!(log.answered[1], "done");
    let urls: Vec<_> = log.sent.iter().map(|(url, _)| url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://oidc.eu-west-1.amazonaws.com/client/register",
            "https://oidc.eu-west-1.amazonaws.com/token",
            "https://q.us-east-1.amazonaws.com/",
            "https://q.eu-central-1.amazonaws.com/",
        ]
    );
    assert!(log.sent[1].1.contains("\"code\":\"org-code\""), "{}", log.sent[1].1);
}

#[test]
fn an_organization_answer_without_its_start_url_fails_in_the_tab_and_the_terminal() {
    let host = ScriptedHost::new(
        [],
        [("/signin/callback", "login_option=awsidc&issuer_url=&amplitude_device_id=&state=r2x32")],
    );
    let error = sign_in(&host).map(|_| ()).err().unwrap_or_else(|| panic!("refused"));
    assert!(error.to_string().contains("issuer_url"), "{error}");
    assert!(host.log.borrow().answered[0].starts_with("failed"), "the tab is answered");
    assert!(host.log.borrow().sent.is_empty());
}

#[test]
fn an_account_without_a_profile_is_told_so() {
    let host = ScriptedHost::new(
        [
            (200, r#"{"clientId":"cid","clientSecret":"secret"}"#.to_owned()),
            (200, r#"{"accessToken":"a","refreshToken":"r"}"#.to_owned()),
            (403, String::new()),
            (200, r#"{"profiles":[]}"#.to_owned()),
        ],
        [("/signin/callback", "login_option=builderid&state=r2x32"), ("/oauth/callback", "code=c&state=r4x32")],
    );
    let error = sign_in(&host).map(|_| ()).err().unwrap_or_else(|| panic!("refused"));
    assert!(error.to_string().contains("no Kiro profile"), "{error}");
}

#[test]
fn a_social_sign_in_exchanges_its_code_with_kiro() {
    let arn_reply = format!(r#"{{"accessToken":"a","refreshToken":"r","profileArn":"{ARN}"}}"#);
    let host = ScriptedHost::new([(200, arn_reply)], [("/oauth/callback", "login_option=google&code=c&state=r2x32")]);
    let tokens = sign_in(&host).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(tokens.account_id, ARN);
    assert_eq!(host.log.borrow().sent[0].0, super::super::auth::TOKEN);
}
