use serde_json::{Value, json};

pub fn brand() -> Value {
    json!({
        "brand": {
            "NAME": brand::NAME,
            "COMMAND": brand::COMMAND,
            "DIR": brand::DIR,
            "INSTRUCTIONS": brand::INSTRUCTIONS,
            "LOCAL_INSTRUCTIONS": brand::LOCAL_INSTRUCTIONS,
            "PLUGIN_DIR": brand::PLUGIN_DIR,
            "ENV": brand::ENV,
            "COMPANY": brand::COMPANY,
        }
    })
}

pub fn path(topic: Option<&str>) -> String {
    let topic = topic.map(str::trim).filter(|topic| !topic.is_empty());
    match topic {
        None => "/docs".to_owned(),
        Some(topic) => format!("/docs?q={}", encoded(topic)),
    }
}

fn encoded(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' => (byte as char).to_string(),
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_learns_every_name_the_docs_spell_as_a_placeholder() {
        let sent = brand();
        assert_eq!(sent["brand"]["NAME"], brand::NAME);
        assert_eq!(sent["brand"]["ENV"], brand::ENV);
        assert_eq!(sent["brand"].as_object().map(|names| names.len()), Some(8));
    }

    #[test]
    fn a_topic_opens_the_page_searching_for_it() {
        assert_eq!(path(None), "/docs");
        assert_eq!(path(Some("  ")), "/docs");
        assert_eq!(path(Some("hooks")), "/docs?q=hooks");
        assert_eq!(path(Some("model roles/ç")), "/docs?q=model%20roles%2F%C3%A7");
    }
}
