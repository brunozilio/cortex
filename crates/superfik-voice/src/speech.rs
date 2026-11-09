//! A reply, as it streams in, cut into sentences worth saying aloud. Pure.
//!
//! Replies are Markdown. Code blocks, tables, links' addresses and the
//! marks around words are for reading, so they are left out; what remains
//! is cut at the end of each sentence (or a long clause) as soon as it has
//! arrived, so the first words are spoken while the rest is still coming.

const MIN_CHARS: usize = 24;
const MAX_CHARS: usize = 220;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sentences {
    pending: String,
    in_code: bool,
    held: String,
}

impl Sentences {
    pub fn push(&mut self, delta: &str) -> Vec<String> {
        self.pending.push_str(delta);
        let mut out = Vec::new();
        while let Some(end) = self.pending.find('\n') {
            let line: String = self.pending.drain(..=end).collect();
            self.line(line.trim_end_matches('\n'), true, &mut out);
        }
        if !self.in_code
            && !self.pending.trim_start().starts_with(['|', '`'])
            && let Some(cut) = last_sentence_end(&self.pending)
        {
            let ready: String = self.pending.drain(..cut).collect();
            self.emit(&clean(&ready), &mut out);
        }
        out
    }

    pub fn finish(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        let rest = std::mem::take(&mut self.pending);
        self.line(&rest, false, &mut out);
        let held = std::mem::take(&mut self.held);
        if !held.trim().is_empty() {
            out.push(held.trim().to_owned());
        }
        *self = Self::default();
        out
    }

    fn line(&mut self, line: &str, whole: bool, out: &mut Vec<String>) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            self.in_code = !self.in_code;
            return;
        }
        if self.in_code || trimmed.starts_with('|') || trimmed.starts_with("    ") && whole && line.starts_with("    ")
        {
            return;
        }
        let text = clean(line);
        if text.is_empty() {
            if let Some(held) = self.take_held() {
                out.push(held);
            }
            return;
        }
        let ended = if text.ends_with(['.', '!', '?', ':', ';']) { text } else { format!("{text}.") };
        self.emit(&ended, out);
    }

    fn take_held(&mut self) -> Option<String> {
        let held = std::mem::take(&mut self.held);
        (!held.trim().is_empty()).then(|| held.trim().to_owned())
    }

    fn emit(&mut self, text: &str, out: &mut Vec<String>) {
        let mut text = std::mem::take(&mut self.held) + " " + text;
        loop {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return;
            }
            let Some(end) = first_sentence_end(trimmed) else {
                self.held = trimmed.to_owned();
                return;
            };
            let (sentence, rest) = trimmed.split_at(end);
            let (sentence, rest) = (sentence.trim().to_owned(), rest.to_owned());
            if sentence.chars().count() < MIN_CHARS && !rest.trim().is_empty() {
                let Some(next) = first_sentence_end(rest.trim()) else {
                    self.held = format!("{sentence} {}", rest.trim());
                    return;
                };
                let rest = rest.trim();
                out.extend(split_long(&format!("{sentence} {}", &rest[..next])));
                text = rest[next..].to_owned();
                continue;
            }
            if sentence.chars().count() < MIN_CHARS {
                self.held = sentence;
                return;
            }
            out.extend(split_long(&sentence));
            text = rest;
        }
    }
}

fn first_sentence_end(text: &str) -> Option<usize> {
    let bytes: Vec<(usize, char)> = text.char_indices().collect();
    for (k, &(at, c)) in bytes.iter().enumerate() {
        if !matches!(c, '.' | '!' | '?' | '…' | ':' | ';') {
            continue;
        }
        let next = bytes.get(k + 1).map(|&(_, n)| n);
        if next.is_none() || next.is_some_and(char::is_whitespace) {
            return Some(at + c.len_utf8());
        }
    }
    None
}

fn last_sentence_end(text: &str) -> Option<usize> {
    let mut end = None;
    let mut from = 0;
    while let Some(at) = first_sentence_end(&text[from..]).filter(|&at| from + at < text.len()) {
        from += at;
        end = Some(from);
    }
    end
}

fn split_long(sentence: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = sentence.trim();
    while rest.chars().count() > MAX_CHARS {
        let limit = rest.char_indices().nth(MAX_CHARS).map_or(rest.len(), |(at, _)| at);
        let Some(comma) = rest[..limit].rfind([',', ';', ' ']) else { break };
        out.push(rest[..=comma].trim().to_owned());
        rest = rest[comma + 1..].trim();
    }
    if !rest.is_empty() {
        out.push(rest.to_owned());
    }
    out
}

fn clean(line: &str) -> String {
    let mut text = line.trim().trim_start_matches(['#', '>']).trim_start().to_owned();
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = text.strip_prefix(bullet) {
            text = rest.to_owned();
        }
    }
    if let Some(at) = text.find(['.', ')']).filter(|&at| at > 0 && text[..at].chars().all(|c| c.is_ascii_digit()))
        && text[at + 1..].starts_with(' ')
    {
        text = text[at + 2..].to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '_' | '`' | '~' => {}
            ']' if chars.peek() == Some(&'(') => {
                for skipped in chars.by_ref() {
                    if skipped == ')' {
                        break;
                    }
                }
            }
            '[' => {}
            _ => out.push(c),
        }
    }
    let words: Vec<&str> =
        out.split_whitespace().filter(|w| !w.starts_with("http://") && !w.starts_with("https://")).collect();
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all(deltas: &[&str]) -> Vec<String> {
        let mut s = Sentences::default();
        let mut out: Vec<String> = deltas.iter().flat_map(|d| s.push(d)).collect();
        out.extend(s.finish());
        out
    }

    #[test]
    fn sentences_leave_as_soon_as_they_end() {
        let mut s = Sentences::default();
        assert!(s.push("Encontrei o problema no").is_empty());
        assert_eq!(s.push(" parser de argumentos. Agora vou"), ["Encontrei o problema no parser de argumentos."]);
        assert!(s.push(" corrigir").is_empty());
        assert_eq!(s.finish(), ["Agora vou corrigir."], "the end of the reply ends the sentence");
    }

    #[test]
    fn short_sentences_join_the_next() {
        assert_eq!(
            all(&["Pronto. ", "Os testes passaram todos sem erro."]),
            ["Pronto. Os testes passaram todos sem erro."]
        );
        assert_eq!(all(&["Ok."]), ["Ok."]);
    }

    #[test]
    fn code_tables_and_marks_are_not_read() {
        let reply = "Mudei **duas** funções em `main.rs`:\n\n```rust\nfn main() {}\n```\n\n| a | b |\n|---|---|\n\
                     - Veja [a documentação](https://docs.rs) para os detalhes completos.\n";
        assert_eq!(
            all(&[reply]),
            ["Mudei duas funções em main.rs:", "Veja a documentação para os detalhes completos."]
        );
    }

    #[test]
    fn decimals_and_versions_do_not_end_a_sentence() {
        assert_eq!(all(&["A versão 2.5 do sherpa ficou mais rápida. "]), ["A versão 2.5 do sherpa ficou mais rápida."]);
    }

    #[test]
    fn long_sentences_are_cut_at_a_comma() {
        let long = format!("{}, {}.", "palavra ".repeat(25).trim(), "outra ".repeat(20).trim());
        let pieces = all(&[&long]);
        assert_eq!(pieces.len(), 2, "{pieces:?}");
        assert!(pieces.iter().all(|p| p.chars().count() <= MAX_CHARS));
    }
}
