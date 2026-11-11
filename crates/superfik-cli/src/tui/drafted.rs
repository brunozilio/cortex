//! What a tool call the model is still writing is about, from its partial
//! JSON input.

/// The command, path or pattern in `json` so far, cut with `…`; empty
/// when none has started.
pub fn drafted(json: &str) -> String {
    for key in ["command", "file_path", "notebook_path", "path", "pattern", "url", "query", "description"] {
        let Some(at) = json.find(&format!("\"{key}\"")) else { continue };
        let rest = json[at + key.len() + 2..].trim_start().trim_start_matches(':').trim_start();
        let Some(rest) = rest.strip_prefix('"') else { continue };
        let mut value = String::new();
        let mut chars = rest.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => match chars.next() {
                    Some('n') => value.push(' '),
                    Some(other) => value.push(other),
                    None => break,
                },
                c => value.push(c),
            }
        }
        return format!("{value}…");
    }
    String::new()
}
