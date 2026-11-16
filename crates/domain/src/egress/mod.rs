use std::collections::HashSet;

const OUTSIDE_TOOLS: [&str; 2] = ["WebFetch", "WebSearch"];
const MCP_PREFIX: &str = "mcp__";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said<'a> {
    User(&'a str),
    Result { tool: &'a str, text: &'a str },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Heard {
    outside: bool,
    addresses: HashSet<String>,
}

impl Heard {
    pub fn of(said: &[Said<'_>]) -> Self {
        let mut heard = Self::default();
        for said in said {
            let (Said::User(text) | Said::Result { text, .. }) = said;
            heard.outside |= matches!(
                said,
                Said::Result { tool, .. } if OUTSIDE_TOOLS.contains(tool) || tool.starts_with(MCP_PREFIX)
            );
            heard.addresses.extend(addresses(text).map(normalized));
        }
        heard
    }

    pub fn concern(&self, url: &str) -> Option<String> {
        (self.outside && !self.addresses.contains(&normalized(url))).then(|| {
            format!(
                "{url} did not appear in the conversation, and this session has read content from outside, which can tell an agent to send data to an address it makes up"
            )
        })
    }
}

fn addresses(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices("http").filter_map(move |(at, _)| {
        let rest = &text[at..];
        if !(rest.starts_with("https://") || rest.starts_with("http://")) {
            return None;
        }
        let end = rest.find(|c: char| c.is_whitespace() || "\"'<>()[]{}`|\\".contains(c)).unwrap_or(rest.len());
        Some(rest[..end].trim_end_matches(['.', ',', ';', ':', '!', '?']))
    })
}

fn normalized(url: &str) -> String {
    let rest = url.trim();
    let rest = rest.strip_prefix("https://").or_else(|| rest.strip_prefix("http://")).unwrap_or(rest);
    let rest = rest.split('#').next().unwrap_or_default();
    let (host, path) = rest.split_once('/').map_or((rest, ""), |(host, path)| (host, path));
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    format!("{host}/{}", path.trim_end_matches('/'))
}

#[cfg(test)]
mod tests;
