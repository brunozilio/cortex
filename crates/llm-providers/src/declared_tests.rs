use serde_json::json;

use super::*;

fn declared(value: Value) -> Declared {
    serde_json::from_value(value).expect("declared")
}

fn groq() -> Declared {
    declared(json!({
        "id": "test-groq",
        "name": "Groq",
        "wire": "chat",
        "base": "https://api.groq.com/openai/v1/",
        "headers": {"x-team": "core"},
    }))
}

#[test]
fn a_declared_provider_is_found_like_a_built_in_one() {
    let id = register(groq()).expect("registered");
    let provider = crate::get(id).expect("found");
    assert_eq!(provider.name(), "Groq");
    assert!(crate::infos().iter().any(|info| info.provider == id), "it can be connected");
    let config = Config { api_key: "gsk-1".into(), ..Config::default() };
    assert_eq!(Call::get(provider, &config, "/models").url, "https://api.groq.com/openai/v1/models");
    let headers = provider.headers(&config);
    assert!(headers.contains(&("authorization".into(), "Bearer gsk-1".into())));
    assert!(headers.contains(&("x-team".into(), "core".into())));
    assert_eq!(provider.path(None), "/chat/completions");
    assert_ne!(provider.slot(), 0);
}

#[test]
fn its_models_come_from_its_list_or_its_catalog() {
    let listed = declared(json!({
        "id": "test-listed", "name": "Listed", "wire": "responses", "base": "http://localhost:8080/v1",
        "models": ["small", "large"], "authHeader": "x-api-key", "authPrefix": "",
    }));
    let provider = crate::get(register(listed).expect("registered")).expect("found");
    let page = provider.page(&json!({})).expect("page");
    let ids: Vec<&str> = page.models.iter().map(|model| model.id.as_str()).collect();
    assert_eq!(ids, ["test-listed:small", "test-listed:large"]);
    let config = Config { api_key: "k".into(), ..Config::default() };
    assert!(provider.headers(&config).contains(&("x-api-key".into(), "k".into())));

    let catalog = crate::get(register(groq()).expect("registered")).expect("found");
    let page = catalog.page(&json!({"data": [{"id": "llama-4"}]})).expect("page");
    assert_eq!(page.models[0].id, "test-groq:llama-4");
}

#[test]
fn a_shaper_rewrites_each_body_and_its_failure_is_a_provider_error() {
    let mut declared = groq();
    declared.id = "test-shaped".into();
    let shaper: Shaper = Box::new(|body: &mut Value| {
        if body["model"] == "bad" {
            return Err("no such model".into());
        }
        body["temperature"] = json!(0.2);
        Ok(())
    });
    let provider = crate::get(register_shaped(declared, Some(shaper)).expect("registered")).expect("found");
    let mut body = json!({"model": "llama"});
    provider.shape(None, &mut body, "medium").expect("shaped");
    assert_eq!(body["temperature"], 0.2);
    let refused = provider.shape(None, &mut json!({"model": "bad"}), "medium").expect_err("refused");
    assert!(refused.to_string().contains("no such model"));
}

#[test]
fn a_bad_declaration_says_what_is_wrong() {
    let refused = |value: Value| register(declared(value)).expect_err("refused").to_string();
    let base = json!({"id": "x", "name": "X", "wire": "chat", "base": "https://x.dev"});
    let with = |key: &str, value: Value| {
        let mut changed = base.clone();
        changed[key] = value;
        changed
    };
    assert!(refused(with("id", json!("Has Space"))).contains("lowercase"));
    assert!(refused(with("id", json!("openai"))).contains("built-in"));
    assert!(refused(with("wire", json!("grpc"))).contains("wire"));
    assert!(refused(with("base", json!("http://evil.example"))).contains("https"));
}
