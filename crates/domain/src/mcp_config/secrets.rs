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

fn secret_name(name: &str) -> bool {
    name.to_ascii_lowercase().split(['-', '_']).any(|word| SECRET_WORDS.contains(&word))
}

fn looks_like_a_token(text: &str) -> bool {
    redact(text, &Known::default()) != text
}
