//! `autofill_credential`: the sign-in with the login the Superfik Vault
//! approved for the run.

use serde_json::json;

use super::*;

type Seen = Arc<Mutex<Vec<(RunId, EngineEvent)>>>;

fn tools() -> (BrowserTools, Seen) {
    let seen: Seen = Arc::default();
    let sink = seen.clone();
    let notify: Notify = Arc::new(move |run, event| lock(&sink).push((run, event)));
    (BrowserTools::new(notify, Duration::from_millis(50)), seen)
}

fn call(name: &str, arguments: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } })
}

struct OneLogin;

impl GrantedLogins for OneLogin {
    fn granted_login(&self, _: &Principal, _: RunId, website: Option<&str>) -> Result<String, String> {
        match website {
            None | Some("https://github.com") => Ok("GitHub".into()),
            Some(_) => Err("no_match: no login is approved for this".into()),
        }
    }
}

#[test]
fn autofill_signs_in_with_the_approved_login_or_says_no_match() {
    let (tools, seen) = tools();
    let tools = tools.with_logins(Arc::new(OneLogin));
    let grant = tools.grant(&Principal::LocalOwner, RunId::new());
    let (answering, seen_by_client) = (tools.clone(), seen.clone());
    let client = std::thread::spawn(move || {
        loop {
            let browse = lock(&seen_by_client).iter().find_map(|(_, e)| match e {
                EngineEvent::Browse { id, op } => Some((id.clone(), op.clone())),
                _ => None,
            });
            if let Some((id, op)) = browse {
                answering.answer(&Principal::LocalOwner, &id, Ok(BrowserAnswer::text("Signed in")));
                return op;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let filled = tools.handle(&grant.token, &call("autofill_credential", json!({}))).expect("ok").expect("answer");
    assert_eq!(client.join().expect("client"), BrowserOp::FillLogin { item: "GitHub".into() });
    assert_eq!(filled["result"]["content"][0]["text"], "Signed in");
    let refused = tools
        .handle(&grant.token, &call("autofill_credential", json!({ "website": "https://gitlab.com" })))
        .expect("authorized")
        .expect("answer");
    assert_eq!(refused["result"]["isError"], true);
    assert!(refused["result"]["content"][0]["text"].as_str().is_some_and(|t| t.starts_with("no_match")));

    let (alone, _) = self::tools();
    let grant = alone.grant(&Principal::LocalOwner, RunId::new());
    let without = alone.handle(&grant.token, &call("autofill_credential", json!({}))).expect("ok").expect("answer");
    assert!(without["result"]["content"][0]["text"].as_str().is_some_and(|t| t.contains("Superfik Vault")));
}
