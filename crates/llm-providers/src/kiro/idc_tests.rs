use super::*;

const ARN: &str = "arn:aws:codewhisperer:us-east-1:111122223333:profile/ABCDEF";

fn client() -> IdcClient {
    IdcClient { region: "eu-west-1".into(), client_id: "cid".into(), client_secret: "secret".into() }
}

fn body(request: &TokenRequest) -> Value {
    serde_json::from_str(&request.body).unwrap_or_default()
}

#[test]
fn the_portal_names_an_organization_s_start_url_and_region() {
    let query = "login_option=awsidc&issuer_url=https%3A%2F%2Facme.awsapps.com%2Fstart&idc_region=eu-west-1&state=s";
    assert_eq!(
        portal_answer("/signin/callback", query),
        Ok(PortalAnswer::Organization {
            start_url: "https://acme.awsapps.com/start".into(),
            region: "eu-west-1".into()
        })
    );
    assert_eq!(portal_answer("/oauth/callback", "login_option=google&code=c&state=s"), Ok(PortalAnswer::Social));
    let builder = portal_answer("/signin/callback", "login_option=builderid&state=s");
    assert_eq!(
        builder,
        Ok(PortalAnswer::Organization {
            start_url: "https://view.awsapps.com/start".into(),
            region: "us-east-1".into()
        })
    );
}

#[test]
fn an_organization_answer_without_its_start_url_or_region_says_so() {
    let missing = portal_answer("/signin/callback", "login_option=awsidc&issuer_url=&amplitude_device_id=");
    assert!(matches!(missing, Err(ProviderError::Unauthorized(why)) if why.contains("issuer_url")));
    let no_region = portal_answer("/signin/callback", "login_option=awsidc&issuer_url=https%3A%2F%2Fa.example");
    assert!(matches!(no_region, Err(ProviderError::Unauthorized(why)) if why.contains("region")));
    let bad = "login_option=awsidc&issuer_url=https%3A%2F%2Fa.example&idc_region=evil.host%2Fx";
    assert!(portal_answer("/signin/callback", bad).is_err(), "a region never becomes part of a host");
    let plain = "login_option=awsidc&issuer_url=http%3A%2F%2Fa.example&idc_region=us-east-1";
    assert!(portal_answer("/signin/callback", plain).is_err(), "the start URL is https");
}

#[test]
fn the_client_registers_with_the_region_and_the_browser_authorizes_there() {
    let request = register("eu-west-1", "https://acme.awsapps.com/start", "http://127.0.0.1:49153/oauth/callback");
    assert_eq!(request.url, "https://oidc.eu-west-1.amazonaws.com/client/register");
    assert_eq!(body(&request)["issuerUrl"], "https://acme.awsapps.com/start");
    assert_eq!(body(&request)["clientName"], "Kiro CLI", "the sign-in page names Kiro");
    assert_eq!(body(&request)["redirectUris"], json!(["http://127.0.0.1:49153/oauth/callback"]));
    assert_eq!(body(&request)["grantTypes"], json!(["authorization_code", "refresh_token"]));
    assert_eq!(registered("eu-west-1", 200, r#"{"clientId":"cid","clientSecret":"secret"}"#), Ok(client()));
    assert!(registered("eu-west-1", 400, "{}").is_err());

    let url = authorize(&client(), "http://127.0.0.1:49153/oauth/callback", "st", "ch");
    assert_eq!(
        url,
        "https://oidc.eu-west-1.amazonaws.com/authorize?response_type=code&client_id=cid\
         &redirect_uri=http%3A%2F%2F127.0.0.1%3A49153%2Foauth%2Fcallback\
         &scopes=codewhisperer%3Acompletions+codewhisperer%3Aanalysis+codewhisperer%3Aconversations\
         &state=st&code_challenge=ch&code_challenge_method=S256"
    );
}

#[test]
fn the_code_and_later_the_refresh_token_go_to_the_region_s_token_endpoint() {
    let exchange = exchange(&client(), "code", "verifier", "http://127.0.0.1:49153/oauth/callback");
    assert_eq!(exchange.url, "https://oidc.eu-west-1.amazonaws.com/token");
    assert_eq!(
        body(&exchange),
        json!({"clientId": "cid", "clientSecret": "secret", "grantType": "authorization_code", "code": "code",
            "redirectUri": "http://127.0.0.1:49153/oauth/callback", "codeVerifier": "verifier"})
    );
    let now = Utc::now();
    let fresh = tokens(&client(), 200, r#"{"accessToken":"a","refreshToken":"r","expiresIn":900}"#, None, now);
    let mut fresh = fresh.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((fresh.access_token.as_str(), fresh.refresh_token.as_str()), ("a", "r"));
    assert_eq!(fresh.expires_at, Some(now + Duration::seconds(900)));
    assert_eq!(fresh.idc, Some(client()), "kept for refreshes");

    fresh.account_id = ARN.into();
    let refresh = refresh(&client(), &fresh);
    assert_eq!(body(&refresh)["grantType"], "refresh_token");
    assert_eq!(body(&refresh)["refreshToken"], "r");
    let renewed = tokens(&client(), 200, r#"{"accessToken":"b"}"#, Some(&fresh), now).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!((renewed.refresh_token.as_str(), renewed.account_id.as_str()), ("r", ARN), "kept from before");
    assert!(tokens(&client(), 400, "{}", Some(&fresh), now).is_err());
}

#[test]
fn the_profile_comes_from_the_first_listed() {
    let (url, headers, _) = profiles("eu-central-1", "tok");
    assert_eq!(url, "https://q.eu-central-1.amazonaws.com/");
    assert!(headers.contains(&("x-amz-target".into(), PROFILES.into())));
    assert!(headers.contains(&("authorization".into(), "Bearer tok".into())));
    let listed = format!(r#"{{"profiles":[{{"arn":"{ARN}","profileName":"Acme"}}]}}"#);
    assert_eq!(first_profile(200, &listed).as_deref(), Some(ARN));
    assert_eq!(first_profile(200, r#"{"profiles":[]}"#), None);
    assert_eq!(first_profile(403, &listed), None);
}
