pub const REDACTED: &str = "[redacted]";

const KNOWN_SHORTEST: usize = 8;

const TOKEN_PREFIXES: &[(&str, usize)] = &[
    ("sk-", 20),
    ("sk_", 20),
    ("ghp_", 20),
    ("gho_", 20),
    ("ghs_", 20),
    ("ghu_", 20),
    ("github_pat_", 20),
    ("glpat-", 20),
    ("xoxb-", 10),
    ("xoxp-", 10),
    ("xoxa-", 10),
    ("xoxs-", 10),
];

const SECRET_KEYS: &[&str] = &["password", "passwd", "secret", "token", "api_key", "apikey", "api-key"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known(Vec<String>);

impl Known {
    pub fn from_env_files<'a>(files: impl IntoIterator<Item = &'a str>) -> Self {
        let mut values: Vec<String> = files.into_iter().flat_map(env_values).collect();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values.dedup();
        Self(values)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

fn env_values(text: &str) -> impl Iterator<Item = String> + '_ {
    text.lines().filter_map(|line| {
        let line = line.trim();
        let line = line.strip_prefix("export ").unwrap_or(line);
        let (key, value) = line.split_once('=')?;
        let value = value.trim().trim_matches(|c| c == '"' || c == '\'');
        let named = !key.trim().is_empty() && !key.trim_start().starts_with('#');
        (named && value.len() >= KNOWN_SHORTEST && !value.contains(char::is_whitespace)).then(|| value.to_owned())
    })
}

pub fn redact(text: &str, known: &Known) -> String {
    let mut out = String::with_capacity(text.len());
    let mut redact_next = false;
    let mut rest = text;
    while !rest.is_empty() {
        let split = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (piece, after) = rest.split_at(split);
        if !piece.is_empty() {
            out.push_str(&word(piece, &mut redact_next));
        }
        let gap = after.find(|c: char| !c.is_whitespace()).unwrap_or(after.len());
        out.push_str(&after[..gap]);
        rest = &after[gap..];
    }
    known.0.iter().fold(out, |text, value| text.replace(value.as_str(), REDACTED))
}

pub(crate) fn word(word: &str, redact_next: &mut bool) -> String {
    if std::mem::take(redact_next) {
        return replace_core(word);
    }
    let core = core(word);
    if is_token(core) {
        replace_core(word)
    } else if let Some(at) = secret_assignment(word) {
        format!("{}{REDACTED}", &word[..at])
    } else {
        *redact_next = core.eq_ignore_ascii_case("bearer") || is_secret_label(word);
        word.to_owned()
    }
}

fn is_edge(c: char) -> bool {
    matches!(c, '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' | '`' | ',' | ';' | '<' | '>') || c == '.'
}

fn core(word: &str) -> &str {
    word.trim_matches(is_edge)
}

fn replace_core(word: &str) -> String {
    let core = core(word);
    if core.is_empty() {
        return word.to_owned();
    }
    let start = word.find(core).unwrap_or(0);
    format!("{}{REDACTED}{}", &word[..start], &word[start + core.len()..])
}

fn is_token(word: &str) -> bool {
    let prefixed = TOKEN_PREFIXES.iter().any(|(prefix, min)| word.starts_with(prefix) && word.len() >= *min);
    prefixed || is_aws_key(word) || is_jwt(word)
}

fn is_aws_key(word: &str) -> bool {
    word.len() == 20
        && (word.starts_with("AKIA") || word.starts_with("ASIA"))
        && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

fn is_jwt(word: &str) -> bool {
    let parts: Vec<&str> = word.split('.').collect();
    parts.len() == 3
        && word.starts_with("eyJ")
        && parts.iter().all(|p| p.len() >= 4 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

fn is_secret_key(key: &str) -> bool {
    let key = key.trim_matches(is_edge).to_ascii_lowercase();
    SECRET_KEYS.iter().any(|secret| key.ends_with(secret))
}

fn secret_assignment(word: &str) -> Option<usize> {
    let at = word.find(['=', ':'])?;
    let (key, value) = (&word[..at], &word[at + 1..]);
    (!core(value).is_empty() && is_secret_key(key)).then_some(at + 1)
}

fn is_secret_label(word: &str) -> bool {
    word.strip_suffix(':').is_some_and(is_secret_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_hidden_and_the_layout_kept() {
        let output = "config:\n    API_KEY=abc123\n\tauth: Bearer eyJhbGc.eyJzdWIi.c2lnbmF0\n  key (ghp_0123456789abcdefghij).\n";

        let shown = redact(output, &Known::default());

        assert_eq!(shown, "config:\n    API_KEY=[redacted]\n\tauth: Bearer [redacted]\n  key ([redacted]).\n");
    }

    #[test]
    fn values_from_env_files_are_hidden_wherever_they_appear() {
        let env = "# comment\nexport DATABASE_URL=\"postgres://u:hunter2hunter@db/x\"\nSHORT=abc\nEMPTY=\nNAME=has spaces in it\n";
        let known = Known::from_env_files([env]);

        let shown = redact("connecting to postgres://u:hunter2hunter@db/x ok, SHORT=abc", &known);

        assert_eq!(shown, "connecting to [redacted] ok, SHORT=abc");
        assert!(Known::from_env_files(["A=b"]).is_empty(), "short values would hide ordinary words");
    }

    #[test]
    fn ordinary_text_is_left_as_it_is() {
        let text = "cargo test --workspace\n  passed: 12, failed: 0\n";
        assert_eq!(redact(text, &Known::default()), text);
    }
}
