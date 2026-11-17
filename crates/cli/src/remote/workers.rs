use serde_json::{Value, json};

pub const COMPATIBILITY_DATE: &str = "2025-09-01";
pub const MIGRATION: &str = "v1";
pub const TOKEN_VARIABLE: &str = "CLOUDFLARE_API_TOKEN";
const API: &str = "https://api.cloudflare.com/client/v4";
const MODULE: &str = "application/javascript+module";
const DATA: &str = "application/octet-stream";
const SUBDOMAIN_HELP: &str = "This Cloudflare account has no workers.dev subdomain yet: open \
https://dash.cloudflare.com, go to Workers & Pages once to pick one, then run this again";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Part {
    pub name: String,
    pub content_type: &'static str,
    pub bytes: Vec<u8>,
}

pub trait Api {
    fn get(&self, path: &str) -> Result<Value, String>;
    fn post(&self, path: &str, body: &Value) -> Result<Value, String>;
    fn put_parts(&self, path: &str, parts: Vec<Part>) -> Result<Value, String>;
}

pub fn script_name() -> String {
    format!("{}-relay", brand::COMMAND)
}

const REMOTE_MARKER: &[u8] = b"remote-relay";

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

pub fn modules(page: &[page::Asset], migrate: bool) -> Result<Vec<Part>, String> {
    if !page::has_index(page) {
        return Err(
            "This program was built without the page: run `cargo run -p xtask -- web`, build again, then run this."
                .into(),
        );
    }
    if !page.iter().any(|asset| asset.path.ends_with(".js") && contains(asset.bytes, REMOTE_MARKER)) {
        return Err("This program's page was built before remote control: run `cargo run -p xtask -- web`, \
             build again, then run this."
            .into());
    }
    let mut imports = String::new();
    let mut entries = Vec::new();
    let mut parts = Vec::new();
    for (at, asset) in page.iter().enumerate() {
        let module = format!("page/{at}.bin");
        imports.push_str(&format!("import p{at} from {};\n", quoted(&format!("./{module}"))));
        entries.push(format!("{}: p{at}", quoted(asset.path)));
        let bytes = if asset.path == "index.html" {
            relay::marked_index(asset.bytes).into_bytes()
        } else {
            asset.bytes.to_vec()
        };
        parts.push(Part { name: module, content_type: DATA, bytes });
    }
    let headers: serde_json::Map<String, Value> =
        relay::SECURITY_HEADERS.iter().map(|(name, value)| ((*name).to_owned(), json!(value))).collect();
    let main = format!(
        "import {{ makeWorker }} from \"./relay.js\";\nexport {{ Relay }} from \"./relay.js\";\n{imports}\
         export default makeWorker({{ page: {{ {} }}, headers: {}, manifest: {} }});\n",
        entries.join(", "),
        Value::Object(headers),
        quoted(&relay::web_manifest()),
    );
    let mut metadata = json!({
        "main_module": "main.js",
        "compatibility_date": COMPATIBILITY_DATE,
        "bindings": [{ "type": "durable_object_namespace", "name": "RELAY", "class_name": "Relay" }],
    });
    if migrate {
        metadata["migrations"] = json!({ "new_tag": MIGRATION, "new_sqlite_classes": ["Relay"] });
    }
    let mut all = vec![
        Part { name: "metadata".into(), content_type: "application/json", bytes: metadata.to_string().into_bytes() },
        Part { name: "main.js".into(), content_type: MODULE, bytes: main.into_bytes() },
        Part { name: "relay.js".into(), content_type: MODULE, bytes: relay::WORKER.as_bytes().to_vec() },
    ];
    all.append(&mut parts);
    Ok(all)
}

fn quoted(text: &str) -> String {
    Value::String(text.to_owned()).to_string()
}

fn only_account(listed: &Value) -> Result<String, String> {
    let accounts = listed.as_array().cloned().unwrap_or_default();
    match accounts.as_slice() {
        [one] => {
            one["id"].as_str().map(str::to_owned).ok_or_else(|| "Cloudflare listed an account without an id.".into())
        }
        [] => Err("The Cloudflare sign-in reaches no account; sign in again and allow the account for Workers.".into()),
        many => {
            let names: Vec<String> = many
                .iter()
                .map(|account| {
                    format!("{} ({})", account["name"].as_str().unwrap_or("?"), account["id"].as_str().unwrap_or("?"))
                })
                .collect();
            Err(format!(
                "The Cloudflare sign-in reaches several accounts; choose one with --account: {}",
                names.join(", ")
            ))
        }
    }
}

pub fn deploy(
    api: &dyn Api,
    account: Option<String>,
    page: &[page::Asset],
    say: &mut dyn FnMut(&str),
) -> Result<String, String> {
    let account = match account {
        Some(account) => account,
        None => only_account(&api.get("/accounts").map_err(|error| {
            format!("Could not list the Cloudflare accounts ({error}); give the account id with --account.")
        })?)?,
    };
    let subdomain = api
        .get(&format!("/accounts/{account}/workers/subdomain"))
        .ok()
        .and_then(|found| found["subdomain"].as_str().map(str::to_owned))
        .ok_or(SUBDOMAIN_HELP)?;
    let name = script_name();
    let scripts = api.get(&format!("/accounts/{account}/workers/scripts"))?;
    let applied = scripts
        .as_array()
        .and_then(|scripts| scripts.iter().find(|script| script["id"].as_str() == Some(&name)))
        .and_then(|script| script["migration_tag"].as_str())
        .is_some_and(|tag| tag == MIGRATION);
    let parts = modules(page, !applied)?;
    say(&format!("Uploading the relay as the Worker {name}…"));
    api.put_parts(&format!("/accounts/{account}/workers/scripts/{name}"), parts)?;
    say("Turning on its workers.dev address…");
    api.post(
        &format!("/accounts/{account}/workers/scripts/{name}/subdomain"),
        &json!({ "enabled": true, "previews_enabled": false }),
    )?;
    Ok(format!("https://{name}.{subdomain}.workers.dev"))
}

#[derive(Debug)]
pub struct Cloudflare {
    token: String,
    http: reqwest::blocking::Client,
}

impl Cloudflare {
    pub fn new(token: String) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .user_agent(format!("{}-remote", brand::COMMAND))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { token, http })
    }

    fn send(&self, request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
        let response = request
            .bearer_auth(&self.token)
            .send()
            .map_err(|error| format!("Cloudflare could not be reached: {error}"))?;
        let status = response.status();
        let body: Value =
            response.json().map_err(|error| format!("Cloudflare answered {status} with no JSON: {error}"))?;
        if body["success"].as_bool() == Some(true) {
            return Ok(body["result"].clone());
        }
        let errors: Vec<String> = body["errors"]
            .as_array()
            .map(|errors| {
                errors
                    .iter()
                    .map(|error| format!("{} ({})", error["message"].as_str().unwrap_or("?"), error["code"]))
                    .collect()
            })
            .unwrap_or_default();
        Err(format!(
            "Cloudflare refused ({status}): {}",
            if errors.is_empty() { body.to_string() } else { errors.join("; ") }
        ))
    }
}

impl Api for Cloudflare {
    fn get(&self, path: &str) -> Result<Value, String> {
        self.send(self.http.get(format!("{API}{path}")))
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.send(self.http.post(format!("{API}{path}")).json(body))
    }

    fn put_parts(&self, path: &str, parts: Vec<Part>) -> Result<Value, String> {
        let mut form = reqwest::blocking::multipart::Form::new();
        for part in parts {
            let file = reqwest::blocking::multipart::Part::bytes(part.bytes)
                .file_name(part.name.clone())
                .mime_str(part.content_type)
                .map_err(|error| error.to_string())?;
            form = form.part(part.name, file);
        }
        self.send(self.http.put(format!("{API}{path}")).multipart(form))
    }
}

#[cfg(test)]
#[path = "workers_tests.rs"]
mod tests;
