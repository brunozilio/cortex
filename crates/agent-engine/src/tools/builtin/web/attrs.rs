use super::html::decode;

pub fn parse(source: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    let mut rest = source.trim_end_matches('/');
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() {
            return attrs;
        }
        let name_end = rest.find(|c: char| c.is_whitespace() || c == '=' || c == '/').unwrap_or(rest.len());
        let name = rest[..name_end].to_ascii_lowercase();
        rest = rest[name_end..].trim_start();
        let Some(after) = rest.strip_prefix('=') else {
            attrs.push((name, String::new()));
            continue;
        };
        let after = after.trim_start();
        let (value, tail) = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                let body = &after[1..];
                let end = body.find(quote).unwrap_or(body.len());
                (&body[..end], body.get(end + 1..).unwrap_or(""))
            }
            _ => {
                let end = after.find(char::is_whitespace).unwrap_or(after.len());
                (&after[..end], &after[end..])
            }
        };
        attrs.push((name, decode(value)));
        rest = tail;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_bare_and_valueless_attributes() {
        assert_eq!(
            parse(r#" HREF="/a?b=1&amp;c=2" class='x y' hidden data-n=3 /"#),
            [
                ("href".to_owned(), "/a?b=1&c=2".to_owned()),
                ("class".to_owned(), "x y".to_owned()),
                ("hidden".to_owned(), String::new()),
                ("data-n".to_owned(), "3".to_owned()),
            ]
        );
    }
}
