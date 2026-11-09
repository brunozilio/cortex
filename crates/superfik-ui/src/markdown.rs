//! Inline markdown, found in place: `code`, **bold**, *italic* and
//! [links](url). Pure; replies drop the markers (`clean`), the composer
//! keeps them and dims them (`Span::open` / `close`).

use std::ops::Range;

use superfik_render::{FontFamily, FontWeight, StyleRun};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Bold,
    Italic,
    Code,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub kind: Kind,
    pub open: Range<usize>,
    pub inner: Range<usize>,
    pub close: Range<usize>,
}

impl Span {
    pub fn whole(&self) -> Range<usize> {
        self.open.start..self.close.end
    }
}

pub fn scan(text: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    scan_in(text, 0..text.len(), &mut spans);
    spans
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(char::is_alphanumeric)
}

fn scan_in(text: &str, range: Range<usize>, out: &mut Vec<Span>) {
    let mut i = range.start;
    while i < range.end {
        let rest = &text[i..range.end];
        let before = text[..i].chars().next_back();
        let found = if rest.starts_with('`') {
            code(text, i, range.end)
        } else if rest.starts_with("**") || (rest.starts_with("__") && !is_word(before)) {
            strong(text, i, range.end)
        } else if rest.starts_with('*') || (rest.starts_with('_') && !is_word(before)) {
            emphasis(text, i, range.end)
        } else if rest.starts_with('[') {
            link(text, i, range.end)
        } else {
            None
        };
        let Some(span) = found else {
            i += rest.chars().next().map_or(1, char::len_utf8);
            continue;
        };
        let next = span.close.end;
        let nested = matches!(span.kind, Kind::Bold | Kind::Italic | Kind::Link).then(|| span.inner.clone());
        out.push(span);
        if let Some(inner) = nested {
            scan_in(text, inner, out);
        }
        i = next;
    }
}

fn code(text: &str, at: usize, end: usize) -> Option<Span> {
    let close = at + 1 + text[at + 1..end].find('`')?;
    (close > at + 1).then(|| Span { kind: Kind::Code, open: at..at + 1, inner: at + 1..close, close: close..close + 1 })
}

fn strong(text: &str, at: usize, end: usize) -> Option<Span> {
    let marker = &text[at..at + 2];
    let start = at + 2;
    let close = start + text[start..end].find(marker)?;
    let inner = &text[start..close];
    let padded = inner.starts_with(char::is_whitespace) || inner.ends_with(char::is_whitespace);
    (!inner.is_empty() && !padded && !(marker == "__" && is_word(text[close + 2..].chars().next()))).then(|| Span {
        kind: Kind::Bold,
        open: at..start,
        inner: start..close,
        close: close..close + 2,
    })
}

fn emphasis(text: &str, at: usize, end: usize) -> Option<Span> {
    let marker = text[at..].chars().next()?;
    let start = at + 1;
    if text[start..end].starts_with(|c: char| c.is_whitespace() || c == marker) {
        return None;
    }
    let mut from = start;
    while let Some(found) = text[from..end].find(marker) {
        let close = from + found;
        let after = text[close + 1..].chars().next();
        let spaced = text[..close].ends_with(char::is_whitespace);
        if !spaced && after != Some(marker) && !(marker == '_' && is_word(after)) {
            return Some(Span { kind: Kind::Italic, open: at..start, inner: start..close, close: close..close + 1 });
        }
        from = close + 1;
    }
    None
}

fn link(text: &str, at: usize, end: usize) -> Option<Span> {
    let label_end = at + 1 + text[at + 1..end].find("](")?;
    let url_end = label_end + 2 + text[label_end + 2..end].find(')')?;
    (label_end > at + 1 && url_end > label_end + 2).then(|| Span {
        kind: Kind::Link,
        open: at..at + 1,
        inner: at + 1..label_end,
        close: label_end..url_end + 1,
    })
}

pub fn runs(spans: &[Span]) -> Vec<StyleRun> {
    spans
        .iter()
        .filter_map(|s| {
            let (weight, family) = match s.kind {
                Kind::Bold => (Some(FontWeight::SemiBold), None),
                Kind::Code => (None, Some(FontFamily::Mono)),
                Kind::Italic | Kind::Link => return None,
            };
            let whole = s.whole();
            Some(StyleRun { start: whole.start, end: whole.end, weight, family })
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Clean {
    pub text: String,
    pub spans: Vec<(Kind, Range<usize>)>,
}

impl Clean {
    pub fn runs(&self) -> Vec<StyleRun> {
        self.spans
            .iter()
            .filter_map(|(kind, r)| {
                let (weight, family) = match kind {
                    Kind::Bold => (Some(FontWeight::SemiBold), None),
                    Kind::Code => (None, Some(FontFamily::Mono)),
                    Kind::Italic | Kind::Link => return None,
                };
                Some(StyleRun { start: r.start, end: r.end, weight, family })
            })
            .collect()
    }

    pub fn kind_at(&self, at: usize) -> Option<Kind> {
        self.spans.iter().rev().find(|(_, r)| r.contains(&at)).map(|(k, _)| *k)
    }
}

pub fn clean(text: &str) -> Clean {
    let spans = scan(text);
    let mut cuts: Vec<Range<usize>> = spans.iter().flat_map(|s| [s.open.clone(), s.close.clone()]).collect();
    cuts.sort_by_key(|c| c.start);
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for cut in &cuts {
        out.push_str(&text[at..cut.start.max(at)]);
        at = cut.end.max(at);
    }
    out.push_str(&text[at..]);
    let moved = |offset: usize| offset - cuts.iter().map(|c| c.end.min(offset).saturating_sub(c.start)).sum::<usize>();
    let spans = spans.iter().map(|s| (s.kind, moved(s.inner.start)..moved(s.inner.end))).collect();
    Clean { text: out, spans }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<(Kind, &str)> {
        scan(text).into_iter().map(|s| (s.kind, &text[s.inner])).collect()
    }

    #[test]
    fn finds_code_bold_italic_and_links() {
        assert_eq!(
            kinds("a `x` **b** *c* [d](http://e)"),
            [(Kind::Code, "x"), (Kind::Bold, "b"), (Kind::Italic, "c"), (Kind::Link, "d")]
        );
    }

    #[test]
    fn code_holds_markers_and_bold_holds_code() {
        assert_eq!(kinds("`**no**`"), [(Kind::Code, "**no**")]);
        assert_eq!(kinds("**run `ls`**"), [(Kind::Bold, "run `ls`"), (Kind::Code, "ls")]);
    }

    #[test]
    fn stray_markers_stay_text() {
        for text in ["a * b", "2 * 3 * 4", "snake_case_name", "open **never closed", "`open", "[a] (b)"] {
            assert!(scan(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn an_unfinished_marker_does_not_hide_a_later_span() {
        assert_eq!(kinds("**open and `code`"), [(Kind::Code, "code")]);
    }

    #[test]
    fn clean_drops_markers_and_moves_the_ranges() {
        let c = clean("Run **`cargo test`** on `main` [now](http://x)");
        assert_eq!(c.text, "Run cargo test on main now");
        let at = |kind| c.spans.iter().filter(|(k, _)| *k == kind).map(|(_, r)| (r.start, r.end)).collect::<Vec<_>>();
        assert_eq!(at(Kind::Bold), [(4, 14)]);
        assert_eq!(at(Kind::Code), [(4, 14), (18, 22)]);
        assert_eq!(at(Kind::Link), [(23, 26)]);
        assert_eq!(c.kind_at(5), Some(Kind::Code));
        assert_eq!(c.kind_at(0), None);
    }

    #[test]
    fn plain_text_is_untouched() {
        assert_eq!(clean("a * b").text, "a * b");
    }

    #[test]
    fn runs_cover_the_markers_so_bytes_line_up() {
        let text = "x **b** `c`";
        let r = runs(&scan(text));
        assert_eq!((r[0].start, r[0].end, r[0].weight), (2, 7, Some(FontWeight::SemiBold)));
        assert_eq!((r[1].start, r[1].end, r[1].family), (8, 11, Some(FontFamily::Mono)));
    }
}
