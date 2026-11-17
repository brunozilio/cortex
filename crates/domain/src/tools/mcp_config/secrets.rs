use serde_json::Value;

use crate::redact::{Known, redact};

const SECRET_WORDS: [&str; 10] =
    ["authorization", "auth", "token", "secret", "password", "passwd", "key", "apikey", "pat", "credentials"];

pub fn written_out(server: &Value) -> Option<String> {
    let named = ["headers", "env"].into_iter().flat_map(|field| {
        server[field].as_object().into_iter().flatten().map(move |(name, value)| (field, name, value))
    });
    for (field, name, value) in named {
        let text = value.as_str().unwrap_or_default();
        if !text.is_empty() && !text.contains("${") && (secret_name(name) || looks_like_a_token(text)) {
            return Some(format!("{field}.{name}"));
        }
    }
    server["args"].as_array().into_iter().flatten().enumerate().find_map(|(at, arg)| {
        let text = arg.as_str().unwrap_or_default();
        (!text.contains("${") && looks_like_a_token(text)).then(|| format!("args[{at}]"))
    })
}

const HIDDEN: &str = "<hidden>";

pub fn redacted(server: &Value) -> Value {
    let mut shown = server.clone();
    for field in ["headers", "env"] {
        for (name, value) in shown.get_mut(field).and_then(Value::as_object_mut).into_iter().flatten() {
            let text = value.as_str().unwrap_or_default();
            if !text.is_empty() && !text.contains("${") && (secret_name(name) || looks_like_a_token(text)) {
                *value = Value::String(HIDDEN.to_owned());
            }
        }
    }
    for arg in shown.get_mut("args").and_then(Value::as_array_mut).into_iter().flatten() {
        let text = arg.as_str().unwrap_or_default();
        if !text.contains("${") && looks_like_a_token(text) {
            *arg = Value::String(HIDDEN.to_owned());
        }
    }
    shown
}

fn secret_name(name: &str) -> bool {
    name.to_ascii_lowercase().split(['-', '_']).any(|word| SECRET_WORDS.contains(&word))
}

fn looks_like_a_token(text: &str) -> bool {
    redact(text, &Known::default()) != text
}
