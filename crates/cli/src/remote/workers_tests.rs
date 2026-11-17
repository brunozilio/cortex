#![allow(clippy::unwrap_used, reason = "a failed step fails the test")]

use std::cell::RefCell;
use std::collections::HashMap;

use super::*;

static PAGE: &[page::Asset] = &[
    page::Asset { path: "index.html", bytes: b"<html><head></head><body></body></html>" },
    page::Asset { path: "assets/app-1.js", bytes: b"app('meta[name=\"remote-relay\"]')" },
    page::Asset { path: "icons/icon-192.png", bytes: &[137, 80, 78, 71] },
];

#[derive(Default)]
struct Fake {
    answers: HashMap<String, Result<Value, String>>,
    asked: RefCell<Vec<String>>,
    uploaded: RefCell<Vec<Part>>,
    posted: RefCell<Vec<Value>>,
}

impl Fake {
    fn answering(mut self, path: &str, answer: Result<Value, String>) -> Self {
        self.answers.insert(path.to_owned(), answer);
        self
    }

    fn usual() -> Self {
        Self::default()
            .answering("/accounts", Ok(json!([{ "id": "acc1", "name": "Mine" }])))
            .answering("/accounts/acc1/workers/subdomain", Ok(json!({ "subdomain": "bruno" })))
            .answering("/accounts/acc1/workers/scripts", Ok(json!([])))
    }

    fn metadata(&self) -> Value {
        let parts = self.uploaded.borrow();
        let part = parts.iter().find(|part| part.name == "metadata").unwrap();
        serde_json::from_slice(&part.bytes).unwrap()
    }
}

impl Api for Fake {
    fn get(&self, path: &str) -> Result<Value, String> {
        self.asked.borrow_mut().push(format!("GET {path}"));
        self.answers.get(path).cloned().unwrap_or_else(|| Err(format!("no answer for {path}")))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.asked.borrow_mut().push(format!("POST {path}"));
        self.posted.borrow_mut().push(body.clone());
        Ok(json!({}))
    }

    fn put_parts(&self, path: &str, parts: Vec<Part>) -> Result<Value, String> {
        self.asked.borrow_mut().push(format!("PUT {path}"));
        *self.uploaded.borrow_mut() = parts;
        Ok(json!({}))
    }
}

fn deployed(fake: &Fake, account: Option<&str>) -> Result<String, String> {
    deploy(fake, account.map(str::to_owned), PAGE, &mut |_| {})
}

#[test]
fn a_first_deploy_uploads_the_worker_with_its_durable_object_and_turns_on_workers_dev() {
    let fake = Fake::usual();
    let url = deployed(&fake, None).unwrap();
    let name = script_name();
    assert_eq!(url, format!("https://{name}.bruno.workers.dev"));
    assert!(fake.asked.borrow().contains(&format!("PUT /accounts/acc1/workers/scripts/{name}")));
    assert!(fake.asked.borrow().contains(&format!("POST /accounts/acc1/workers/scripts/{name}/subdomain")));
    assert_eq!(fake.posted.borrow()[0], json!({ "enabled": true, "previews_enabled": false }));
    let metadata = fake.metadata();
    assert_eq!(metadata["main_module"], "main.js");
    assert_eq!(
        metadata["bindings"][0],
        json!({ "type": "durable_object_namespace", "name": "RELAY", "class_name": "Relay" })
    );
    assert_eq!(metadata["migrations"], json!({ "new_tag": MIGRATION, "new_sqlite_classes": ["Relay"] }));
}

#[test]
fn a_second_deploy_does_not_repeat_the_migration() {
    let fake = Fake::usual()
        .answering("/accounts/acc1/workers/scripts", Ok(json!([{ "id": script_name(), "migration_tag": MIGRATION }])));
    deployed(&fake, None).unwrap();
    assert!(fake.metadata().get("migrations").is_none());
}

#[test]
fn the_worker_carries_the_page_with_the_relay_marker_and_the_app_s_manifest() {
    let parts = modules(PAGE, true).unwrap();
    let names: Vec<&str> = parts.iter().map(|part| part.name.as_str()).collect();
    assert_eq!(names, ["metadata", "main.js", "relay.js", "page/0.bin", "page/1.bin", "page/2.bin"]);
    let main = String::from_utf8_lossy(&parts[1].bytes).into_owned();
    assert!(main.contains("import p2 from \"./page/2.bin\";"));
    assert!(main.contains("\"icons/icon-192.png\": p2"));
    assert!(main.contains("export { Relay } from \"./relay.js\";"));
    assert!(main.contains(&serde_json::Value::String(relay::web_manifest()).to_string()));
    assert!(main.contains("content-security-policy"));
    assert!(String::from_utf8_lossy(&parts[3].bytes).contains(relay::MARKER));
    assert_eq!(parts[5].bytes, [137, 80, 78, 71]);
    assert_eq!(parts[2].bytes, relay::WORKER.as_bytes());
}

#[test]
fn a_page_built_before_remote_control_is_not_deployed() {
    let old: &[page::Asset] = &[
        page::Asset { path: "index.html", bytes: b"<html><head></head></html>" },
        page::Asset { path: "assets/app-0.js", bytes: b"fetch('/api/report')" },
    ];
    let refused = modules(old, true).unwrap_err();
    assert!(refused.contains("before remote control") && refused.contains("xtask -- web"), "{refused}");
}

#[test]
fn a_program_built_without_the_page_has_nothing_to_deploy() {
    assert!(modules(&[], true).unwrap_err().contains("xtask -- web"));
}

#[test]
fn a_token_reaching_several_accounts_asks_which_and_a_given_account_skips_the_listing() {
    let fake = Fake::usual()
        .answering("/accounts", Ok(json!([{ "id": "acc1", "name": "Mine" }, { "id": "acc2", "name": "Work" }])));
    let refused = deployed(&fake, None).unwrap_err();
    assert!(refused.contains("--account") && refused.contains("Work (acc2)"), "{refused}");
    deployed(&fake, Some("acc1")).unwrap();
    assert!(!fake.asked.borrow().iter().skip(1).any(|asked| asked == "GET /accounts"));
}

#[test]
fn an_account_without_a_workers_dev_subdomain_is_told_where_to_pick_one() {
    let fake = Fake::usual().answering("/accounts/acc1/workers/subdomain", Err("not found".into()));
    let refused = deployed(&fake, None).unwrap_err();
    assert!(refused.contains("Workers & Pages"), "{refused}");
    assert!(fake.uploaded.borrow().is_empty());
}
