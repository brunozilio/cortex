//! The Chrome DevTools Protocol as the browser card speaks it, for vault
//! logins: messages out, replies and events in, and the frames a page is
//! made of. Our scripts run in an isolated world ([`WORLD`]), so the page's
//! own scripts can neither see nor change them, and the capture script
//! reaches the app through a binding ([`BINDING`]) that only that world
//! has. Pure.

use serde_json::{Value, json};

pub const WORLD: &str = "superfik";
pub const BINDING: &str = "__superfikLogin";
const FIRST_ID: u64 = 500_000_000;

#[derive(Debug)]
pub struct Cdp {
    next: u64,
}

impl Default for Cdp {
    fn default() -> Self {
        Self { next: FIRST_ID }
    }
}

impl Cdp {
    pub fn command(&mut self, method: &str, params: Value) -> (u64, String) {
        let id = self.next;
        self.next += 1;
        (id, json!({ "id": id, "method": method, "params": params }).to_string())
    }

    pub fn setup(&mut self, capture_script: &str, passkey_script: &str) -> Vec<String> {
        [
            self.command("Runtime.enable", json!({})),
            self.command("Runtime.addBinding", json!({ "name": BINDING, "executionContextName": WORLD })),
            self.command(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": capture_script, "worldName": WORLD, "runImmediately": true }),
            ),
            self.command("Runtime.addBinding", json!({ "name": crate::passkey::BINDING })),
            self.command(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": passkey_script, "runImmediately": true }),
            ),
            self.command("Page.enable", json!({})),
        ]
        .into_iter()
        .map(|(_, message)| message)
        .collect()
    }

    pub fn frame_tree(&mut self) -> (u64, String) {
        self.command("Page.getFrameTree", json!({}))
    }

    pub fn isolated_world(&mut self, frame: &str) -> (u64, String) {
        self.command("Page.createIsolatedWorld", json!({ "frameId": frame, "worldName": WORLD }))
    }

    pub fn evaluate(&mut self, context: i64, expression: &str) -> (u64, String) {
        self.command(
            "Runtime.evaluate",
            json!({ "expression": expression, "contextId": context, "returnByValue": true }),
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    Reply { id: u64, result: Result<Value, String> },
    Event { method: String, params: Value },
}

pub fn incoming(json: &str) -> Option<Incoming> {
    let value: Value = serde_json::from_str(json).ok()?;
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let result = match value.get("error") {
            Some(error) => Err(error.get("message").and_then(Value::as_str).unwrap_or("failed").to_owned()),
            None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
        };
        return Some(Incoming::Reply { id, result });
    }
    let method = value.get("method").and_then(Value::as_str)?.to_owned();
    Some(Incoming::Event { method, params: value.get("params").cloned().unwrap_or(Value::Null) })
}

pub fn frames(reply: &Value) -> Vec<(String, String)> {
    fn walk(node: &Value, out: &mut Vec<(String, String)>) {
        let frame = &node["frame"];
        if let (Some(id), Some(url)) = (frame["id"].as_str(), frame["url"].as_str()) {
            out.push((id.to_owned(), url.to_owned()));
        }
        for child in node["childFrames"].as_array().into_iter().flatten() {
            walk(child, out);
        }
    }
    let mut out = Vec::new();
    walk(&reply["frameTree"], &mut out);
    out
}

pub fn context(reply: &Value) -> Option<i64> {
    reply["executionContextId"].as_i64()
}

pub fn value(reply: &Value) -> Result<String, String> {
    if let Some(exception) = reply.get("exceptionDetails") {
        let text = exception["exception"]["description"].as_str().or(exception["text"].as_str());
        return Err(text.unwrap_or("the script threw").to_owned());
    }
    Ok(reply["result"].get("value").cloned().unwrap_or(Value::Null).to_string())
}

pub fn passkey_payload(method: &str, params: &Value) -> Option<(i64, String)> {
    if method != "Runtime.bindingCalled" || params["name"] != crate::passkey::BINDING {
        return None;
    }
    Some((params["executionContextId"].as_i64()?, params["payload"].as_str()?.to_owned()))
}

pub fn top_context(method: &str, params: &Value) -> Option<(i64, String)> {
    let context = &params["context"];
    let main = context["auxData"]["isDefault"].as_bool() == Some(true);
    (method == "Runtime.executionContextCreated" && main)
        .then(|| Some((context["id"].as_i64()?, context["origin"].as_str()?.to_owned())))
        .flatten()
}

pub fn binding_payload(method: &str, params: &Value) -> Option<String> {
    (method == "Runtime.bindingCalled" && params["name"] == BINDING)
        .then(|| params["payload"].as_str().map(str::to_owned))
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_numbered_apart_from_weldings() {
        let mut cdp = Cdp::default();
        let (first, message) = cdp.command("Page.getFrameTree", json!({}));
        let (second, _) = cdp.command("Page.getFrameTree", json!({}));
        assert!(first >= FIRST_ID && second == first + 1);
        let sent: Value = serde_json::from_str(&message).expect("json");
        assert_eq!((sent["id"].as_u64(), sent["method"].as_str()), (Some(first), Some("Page.getFrameTree")));
    }

    #[test]
    fn the_setup_binds_our_world_only_and_captures_every_document() {
        let sent: Vec<Value> = Cdp::default()
            .setup("capture()", "shim()")
            .iter()
            .map(|m| serde_json::from_str(m).expect("json"))
            .collect();
        assert_eq!(sent[1]["params"], json!({ "name": BINDING, "executionContextName": WORLD }));
        assert_eq!(sent[2]["params"]["worldName"], WORLD);
        assert_eq!(sent[2]["params"]["runImmediately"], true);
        assert_eq!(sent[3]["params"], json!({ "name": crate::passkey::BINDING }), "the page's own world");
        assert_eq!((sent[4]["params"]["source"].as_str(), sent[4]["params"].get("worldName")), (Some("shim()"), None));
    }

    #[test]
    fn passkey_requests_name_the_context_they_came_from() {
        let created =
            json!({ "context": { "id": 4, "origin": "https://google.com", "auxData": { "isDefault": true } } });
        assert_eq!(top_context("Runtime.executionContextCreated", &created), Some((4, "https://google.com".into())));
        let isolated =
            json!({ "context": { "id": 5, "origin": "https://google.com", "auxData": { "isDefault": false } } });
        assert_eq!(top_context("Runtime.executionContextCreated", &isolated), None, "our own world");
        let called = json!({ "name": crate::passkey::BINDING, "executionContextId": 4, "payload": "{}" });
        assert_eq!(passkey_payload("Runtime.bindingCalled", &called), Some((4, "{}".into())));
        let login = json!({ "name": BINDING, "executionContextId": 4, "payload": "{}" });
        assert_eq!(passkey_payload("Runtime.bindingCalled", &login), None);
    }

    #[test]
    fn replies_events_and_frames_read_back() {
        let reply = incoming(r#"{"id":7,"result":{"executionContextId":12}}"#).expect("reply");
        let Incoming::Reply { id: 7, result: Ok(result) } = reply else { panic!("{reply:?}") };
        assert_eq!(context(&result), Some(12));
        assert!(matches!(incoming(r#"{"id":8,"error":{"message":"No frame"}}"#),
            Some(Incoming::Reply { id: 8, result: Err(e) }) if e == "No frame"));
        let tree = json!({ "frameTree": {
            "frame": { "id": "top", "url": "https://a.com/" },
            "childFrames": [{ "frame": { "id": "login", "url": "https://login.a.com/" } }]
        } });
        assert_eq!(
            frames(&tree),
            [("top".into(), "https://a.com/".into()), ("login".into(), "https://login.a.com/".into())]
        );
        assert_eq!(
            value(&json!({ "result": { "type": "object", "value": { "filled": 1 } } })),
            Ok(r#"{"filled":1}"#.into())
        );
        assert!(value(&json!({ "exceptionDetails": { "text": "Uncaught" } })).is_err());
        let event = json!({ "name": BINDING, "payload": "{}" });
        assert_eq!(binding_payload("Runtime.bindingCalled", &event), Some("{}".into()));
        assert_eq!(binding_payload("Runtime.bindingCalled", &json!({ "name": "other", "payload": "{}" })), None);
    }
}
