use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    fields: HashMap<String, String>,
}

impl Frontmatter {
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, String)>) -> Self {
        Self { fields: pairs.into_iter().collect() }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str).filter(|value| !value.is_empty())
    }
}

pub fn parse(text: &str) -> (Frontmatter, &str) {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some((block, body)) = split_block(text) else { return (Frontmatter::default(), text) };
    let mut fields = HashMap::new();
    let mut current: Option<String> = None;
    for line in block.lines() {
        let continued = line.starts_with([' ', '\t']);
        match current.as_ref().filter(|_| continued) {
            Some(key) => append(&mut fields, key, line.trim()),
            None => {
                current = parse_pair(line).map(|(key, value)| {
                    fields.insert(key.clone(), value);
                    key
                })
            }
        }
    }
    (Frontmatter { fields }, body)
}

fn split_block(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("---")?.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n'))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some((&rest[..offset], &rest[offset + line.len()..]));
        }
        offset += line.len();
    }
    None
}

fn parse_pair(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || key.starts_with('#') || key.contains(char::is_whitespace) {
        return None;
    }
    let value = value.trim();
    let value = if matches!(value, ">" | "|" | ">-" | "|-") { "" } else { unquote(value) };
    Some((key.to_ascii_lowercase(), value.to_owned()))
}

fn append(fields: &mut HashMap<String, String>, key: &str, line: &str) {
    if line.is_empty() {
        return;
    }
    if let Some(value) = fields.get_mut(key) {
        if !value.is_empty() {
            value.push(' ');
        }
        value.push_str(line);
    }
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value.strip_prefix(quote).and_then(|v| v.strip_suffix(quote)) {
            return inner;
        }
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_pairs_and_body() {
        let (front, body) = parse("---\nname: motion\nDescription: \"Animate things: nicely\"\n---\n# Motion\n");
        assert_eq!(front.get("name"), Some("motion"));
        assert_eq!(front.get("description"), Some("Animate things: nicely"));
        assert_eq!(body, "# Motion\n");
    }

    #[test]
    fn joins_continued_and_block_values() {
        let (front, _) = parse("---\ndescription: >\n  first line\n  second line\nname: 'x'\n---\n");
        assert_eq!(front.get("description"), Some("first line second line"));
        assert_eq!(front.get("name"), Some("x"));
    }

    #[test]
    fn tolerates_crlf_bom_and_junk() {
        let (front, _) = parse("\u{feff}---\r\nname: a\r\n# comment: here\r\nnot a pair\r\n  stray\r\n---\r\nbody");
        assert_eq!(front.get("name"), Some("a"));
        assert_eq!(front.get("# comment"), None);
    }

    #[test]
    fn missing_or_unterminated_block_is_empty() {
        assert_eq!(parse("# Title\nname: x").0, Frontmatter::default());
        let unterminated = "---\nname: x\n";
        assert_eq!(parse(unterminated), (Frontmatter::default(), unterminated));
        assert_eq!(parse("---\nname:\n---\n").0.get("name"), None);
    }
}
