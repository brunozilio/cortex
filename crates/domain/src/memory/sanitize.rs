pub const REDACTED: &str = "[redacted]";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sanitized(String);

impl Sanitized {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

pub fn sanitize(text: &str) -> Sanitized {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut redact_next = false;
    for word in words {
        if redact_next {
            redact_next = false;
            out.push(replace_core(word));
            continue;
        }
        let core = core(word);
        if is_token(core) {
            out.push(replace_core(word));
        } else if let Some(at) = secret_assignment(word) {
            out.push(format!("{}{REDACTED}", &word[..at]));
        } else {
            redact_next = core.eq_ignore_ascii_case("bearer") || is_secret_label(word);
            out.push(word.to_owned());
        }
    }
    Sanitized(out.join(" "))
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
