use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tui_markdown::{BuiltinCodeTheme, Options, StyleSheet};

use super::wrap::wrap_line;

const FENCE: &str = "─── ";
const KEPT: usize = 256;

thread_local! {
    static RENDERED: RefCell<HashMap<Key, Kept>> = RefCell::new(HashMap::new());
}

type Key = (u64, usize, bool);

struct Kept {
    text: String,
    lines: Vec<Line<'static>>,
}

fn key(text: &str, width: usize, light: bool) -> Key {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    (hasher.finish(), width, light)
}

fn kept(key: Key, text: &str) -> Option<Vec<Line<'static>>> {
    RENDERED.with(|kept| kept.borrow().get(&key).filter(|kept| kept.text == text).map(|kept| kept.lines.clone()))
}

fn keep(key: Key, text: &str, lines: &[Line<'static>]) {
    RENDERED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(key, Kept { text: text.to_owned(), lines: lines.to_vec() });
    });
}

#[derive(Debug, Clone, Copy)]
struct Look;

impl StyleSheet for Look {
    fn heading(&self, level: u8) -> Style {
        let style = Style::new().add_modifier(Modifier::BOLD);
        if level == 1 { style.add_modifier(Modifier::UNDERLINED) } else { style }
    }

    fn heading_marker(&self, _level: u8) -> &str {
        ""
    }

    fn code(&self) -> Style {
        Style::new().fg(Color::Cyan)
    }

    fn link(&self) -> Style {
        Style::new().fg(Color::Blue).add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::new().fg(Color::Gray).add_modifier(Modifier::ITALIC)
    }

    fn code_block_fence(&self) -> &str {
        FENCE
    }

    fn list_marker(&self) -> Style {
        Style::new().fg(Color::Magenta)
    }

    fn table_border(&self) -> Style {
        Style::new().fg(Color::DarkGray)
    }

    fn table_header(&self) -> Style {
        Style::new().add_modifier(Modifier::BOLD)
    }
}

pub fn render(text: &str, width: usize) -> Vec<Line<'static>> {
    let theme = super::theme::current();
    let key = key(text, width, theme.light());
    if let Some(kept) = kept(key, text) {
        return kept;
    }
    let code = if theme.light() { BuiltinCodeTheme::InspiredGitHub } else { BuiltinCodeTheme::Base16OceanDark };
    let options = Options::new(Look).table_width(u16::try_from(width).unwrap_or(u16::MAX)).code_theme(code);
    let drawn = tui_markdown::from_str_with_options(text, &options).lines.into_iter().map(owned).collect();
    let band = theme.code_bg();
    let lines: Vec<Line<'static>> = dress(drawn, band)
        .into_iter()
        .flat_map(|line| wrap_line(line, width))
        .map(|mut line| {
            if line.style.bg == Some(band) {
                line.spans.push(Span::raw(" ".repeat(width.saturating_sub(line.width()))));
            }
            line
        })
        .collect();
    keep(key, text, &lines);
    lines
}

const BAR: &str = "│ ";

fn dress(lines: Vec<Line<'static>>, code_bg: Color) -> Vec<Line<'static>> {
    let faint = Style::new().fg(Color::DarkGray);
    let mut out = Vec::with_capacity(lines.len());
    let mut in_code = false;
    for mut line in lines {
        let fence = line.spans.first().and_then(|span| span.content.strip_prefix(FENCE.trim_end()));
        if let Some(rest) = fence {
            let language = rest.trim().to_owned();
            if !in_code && !language.is_empty() {
                out.push(Line::styled(language, faint));
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            line.spans.insert(0, Span::styled(BAR, faint));
            line = line.patch_style(Style::new().bg(code_bg));
        } else if let Some(first) = line.spans.first_mut()
            && first.content.trim_start() == "- "
        {
            first.content = first.content.replacen('-', "•", 1).into();
            first.style = faint;
        }
        out.push(line);
    }
    out
}

fn owned(line: Line<'_>) -> Line<'static> {
    let spans: Vec<Span<'static>> =
        line.spans.into_iter().map(|span| Span::styled(span.content.into_owned(), span.style)).collect();
    Line::from(spans).style(line.style)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
    }

    #[test]
    fn headings_lists_and_code_are_drawn_and_wrapped() {
        let lines = render("## Plan\n\n- run **all** the tests\n\n```rust\nfn main() {}\n```", 14);
        let text = plain(&lines);
        assert_eq!(text[0], "Plan", "no # marker: {text:?}");
        assert!(lines[0].spans[0].style.patch(lines[0].style).add_modifier.contains(Modifier::BOLD));
        assert!(text.iter().any(|l| l.starts_with("• run all")), "a bullet, not a dash: {text:?}");
        assert!(text.iter().any(|l| l == "rust"), "the language labels the block: {text:?}");
        assert!(text.iter().any(|l| l.trim_end() == "│ fn main() {}"), "code sits behind a bar: {text:?}");
        let code = lines.iter().find(|l| plain(std::slice::from_ref(l))[0].starts_with('│')).expect("code");
        assert_eq!(code.width(), 14, "the block's band spans the width");
        assert!(code.style.bg.is_some(), "and is tinted");
        assert!(!text.iter().any(|l| l.starts_with('─')), "no fence rules: {text:?}");
        assert!(lines.iter().all(|l| l.width() <= 14), "{text:?}");
    }

    #[test]
    fn a_render_is_kept_for_the_same_text_and_width() {
        let first = render("kept **once**", 40);
        assert_eq!(render("kept **once**", 40), first);
        let light = super::super::theme::current().light();
        assert!(kept(key("kept **once**", 40, light), "kept **once**").is_some());
    }

    #[test]
    fn a_text_that_only_shares_the_hash_of_a_kept_one_is_rendered_itself() {
        let light = super::super::theme::current().light();
        let taken = key("the other text", 40, light);
        keep(taken, "some text with the same hash", &[Line::from("stale")]);
        assert!(kept(taken, "the other text").is_none(), "the kept text is compared, not just its hash");
        assert_eq!(plain(&render("the other text", 40)), ["the other text"]);
    }
}
