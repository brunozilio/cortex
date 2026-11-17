use std::ops::Range;

const RUNS_A_COMMAND: [&str; 4] = ["-exec", "-execdir", "-ok", "-okdir"];

fn value(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().filter(|c| !matches!(c, '\'' | '"' | '\\')).collect()
}

pub(super) fn commands(bytes: &[u8], words: &[Range<usize>]) -> Vec<Range<usize>> {
    let values: Vec<String> = words.iter().map(|word| value(&bytes[word.clone()])).collect();
    let Some(find) = values.iter().position(|word| word.rsplit('/').next() == Some("find")) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut at = find + 1;
    while at < values.len() {
        if !RUNS_A_COMMAND.contains(&values[at].as_str()) {
            at += 1;
            continue;
        }
        let first = at + 1;
        let end =
            values[first..].iter().position(|word| word == ";" || word == "+").map_or(values.len(), |at| first + at);
        if end > first {
            found.push(words[first].start..words[end - 1].end);
        }
        at = end + 1;
    }
    found
}
