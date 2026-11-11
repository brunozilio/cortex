use crate::transcript::{Block, Message, Role};

pub(super) const TITLE_CHARS: usize = 60;

pub fn conversation_text(history: &[Message], limit: usize) -> String {
    let mut text = String::new();
    for message in history {
        let words: Vec<&str> = message
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Text(text) => Some(text.trim()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect();
        if words.is_empty() {
            continue;
        }
        let who = if message.role == Role::User { "User" } else { "Agent" };
        text.push_str(&format!("{who}: {}\n", words.join("\n")));
    }
    let text = text.trim();
    let chars = text.chars().count();
    text.chars().skip(chars.saturating_sub(limit)).collect::<String>().trim().to_owned()
}

pub(super) fn field(answer: &str, field: &str) -> String {
    let parsed = answer
        .find('{')
        .zip(answer.rfind('}'))
        .filter(|(start, end)| start < end)
        .and_then(|(start, end)| serde_json::from_str::<serde_json::Value>(&answer[start..=end]).ok());
    match parsed.as_ref().and_then(|value| value[field].as_str()) {
        Some(value) => value.trim().to_owned(),
        None => answer.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default().to_owned(),
    }
}

pub fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().next().unwrap_or_default().trim().trim_matches(['"', '\'', '`', '*', '#']).trim();
    let line = line.strip_suffix('.').unwrap_or(line).trim();
    let mut chars = line.chars();
    let first = chars.next()?;
    let title: String = first.to_uppercase().chain(chars).take(TITLE_CHARS).collect();
    Some(title)
}

pub fn kebab(raw: &str) -> Option<String> {
    let words: Vec<String> = raw
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(4)
        .map(str::to_ascii_lowercase)
        .collect();
    (!words.is_empty()).then(|| words.join("-"))
}

pub fn spoken(kebab: &str) -> String {
    let words = kebab.replace('-', " ");
    let mut chars = words.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

pub fn clean_suggestion(raw: &str) -> Option<String> {
    let line = raw.trim().trim_matches(['"', '\'', '`', '“', '”']).trim();
    let words = line.split_whitespace().count();
    let lower = line.to_lowercase();
    let voice = ["let me", "i'll", "i will", "here's", "here is", "looks good", "thanks", "thank you"];
    let refused = line.is_empty()
        || line.contains('\n')
        || line.contains('?')
        || !(1..=12).contains(&words)
        || line.matches(". ").count() > 0
        || voice.iter().any(|start| lower.starts_with(start))
        || lower.starts_with('[')
        || lower == "nothing"
        || lower == "silence";
    (!refused).then(|| line.to_owned())
}
