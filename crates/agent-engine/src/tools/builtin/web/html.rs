pub fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.char_indices().take(12).find(|(_, c)| *c == ';').map(|(end, _)| end);
        match end.and_then(|end| entity(&rest[1..end]).map(|c| (end, c))) {
            Some((end, c)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        "hellip" => Some('…'),
        "copy" => Some('©'),
        _ => name
            .strip_prefix("#x")
            .or_else(|| name.strip_prefix("#X"))
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
            .and_then(char::from_u32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entities_decode_and_strangers_stay() {
        assert_eq!(decode("a &amp; b&nbsp;&#x2764;&#39; &mdash; &bogus; & &ação;çã"), "a & b ❤' — &bogus; & &ação;çã");
    }
}
