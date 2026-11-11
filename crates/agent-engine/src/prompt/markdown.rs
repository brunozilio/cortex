use serde_json::Value;
pub fn frontmatter(text: &str) -> Result<(Value, &str), String> {
    let text = text.trim_start_matches('\u{feff}');
    let Some(first) = text.find('\n') else {
        return Ok((Value::Null, text));
    };
    if text[..first].trim_end() != "---" {
        return Ok((Value::Null, text));
    }
    let mut offset = first + 1;
    for line in text[offset..].split_inclusive('\n') {
        if line.trim_end() == "---" {
            let header = &text[first + 1..offset];
            if header.len() > 65536 {
                return Err("frontmatter exceeds the metadata limit".into());
            }
            let value =
                serde_saphyr::from_str::<Value>(header).map_err(|e| format!("invalid YAML frontmatter: {e}"))?;
            if !value.is_null() && !value.is_object() {
                return Err("frontmatter must be a mapping".into());
            }
            return Ok((value, &text[offset + line.len()..]));
        }
        offset += line.len();
    }
    Err("unterminated YAML frontmatter".into())
}
pub fn list(value: &Value) -> Result<Option<Vec<String>>, String> {
    match value {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(split_patterns(text))),
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_owned).ok_or_else(|| "metadata list must contain strings".into()))
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        _ => Err("metadata must be a list or comma-separated string".into()),
    }
}
fn split_patterns(text: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, c) in text.char_indices() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(text[start..at].trim().to_owned());
                start = at + 1;
            }
            _ => {}
        }
    }
    items.push(text[start..].trim().to_owned());
    items.into_iter().filter(|s| !s.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::frontmatter;
    use serde_json::json;

    #[test]
    fn parses_a_mapping_and_returns_the_body() {
        let text =
            "\u{feff}---\nname: reviewer\nmax_turns: 3\nstrict: true\ntools: [Read, Grep]\n# comment\n---\nBody\n";
        let (value, body) = frontmatter(text).expect("valid frontmatter");
        assert_eq!(value, json!({"name": "reviewer", "max_turns": 3, "strict": true, "tools": ["Read", "Grep"]}));
        assert_eq!(body, "Body\n");
    }

    #[test]
    fn empty_frontmatter_is_null() {
        let (value, body) = frontmatter("---\n---\nBody").expect("empty frontmatter");
        assert!(value.is_null());
        assert_eq!(body, "Body");
        let (value, _) = frontmatter("---\n# only a comment\n---\nBody").expect("comment-only frontmatter");
        assert!(value.is_null());
    }

    #[test]
    fn text_without_frontmatter_is_all_body() {
        let (value, body) = frontmatter("Just text\n---\n").expect("no frontmatter");
        assert!(value.is_null());
        assert_eq!(body, "Just text\n---\n");
    }

    #[test]
    fn rejects_scalars_unterminated_and_invalid_yaml() {
        assert!(frontmatter("---\njust a string\n---\nBody").is_err());
        assert!(frontmatter("---\nname: x\nBody").is_err());
        assert!(frontmatter("---\npaths: [unterminated\n---\nBody").is_err());
    }

    #[test]
    fn block_scalars_keep_their_text() {
        let (value, _) = frontmatter("---\ndescription: |\n  line one\n  line two\n---\n").expect("block scalar");
        assert_eq!(value["description"], "line one\nline two\n");
    }
}
