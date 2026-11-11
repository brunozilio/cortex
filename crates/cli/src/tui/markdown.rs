use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use tui_markdown::{Options, StyleSheet};

use super::wrap::wrap_line;

const FENCE: &str = "─── ";
const KEPT: usize = 256;

thread_local! {
    static RENDERED: RefCell<HashMap<(u64, usize), Vec<Line<'static>>>> = RefCell::new(HashMap::new());
}

#[derive(Debug, Clone, Copy)]
struct Look;

impl StyleSheet for Look {
    fn heading(&self, level: u8) -> Style {
        let style = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
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
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    let key = (hasher.finish(), width);
    if let Some(kept) = RENDERED.with(|kept| kept.borrow().get(&key).cloned()) {
        return kept;
    }
    let options = Options::new(Look).table_width(u16::try_from(width).unwrap_or(u16::MAX));
    let lines: Vec<Line<'static>> = tui_markdown::from_str_with_options(text, &options)
        .lines
        .into_iter()
        .map(|line| {
            let line = owned(line);
            let fence = line.spans.first().is_some_and(|span| span.content.starts_with(FENCE.trim_end()));
            if fence { line.style(Style::new().fg(Color::DarkGray)) } else { line }
        })
        .flat_map(|line| wrap_line(line, width))
        .collect();
    RENDERED.with(|kept| {
        let mut kept = kept.borrow_mut();
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(key, lines.clone());
    });
    lines
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
        assert_eq!(lines[0].spans[0].style.fg.or(lines[0].style.fg), Some(Color::Magenta));
        assert!(text.iter().any(|l| l.contains("run all the")), "{text:?}");
        assert!(text.iter().any(|l| l == "─── rust"), "{text:?}");
        assert!(text.iter().any(|l| l.contains("fn main() {}")), "{text:?}");
        assert!(lines.iter().all(|l| l.width() <= 14), "{text:?}");
    }

    #[test]
    fn a_render_is_kept_for_the_same_text_and_width() {
        let first = render("kept **once**", 40);
        assert_eq!(render("kept **once**", 40), first);
        assert!(RENDERED.with(|kept| kept.borrow().contains_key(&{
            let mut h = DefaultHasher::new();
            "kept **once**".hash(&mut h);
            (h.finish(), 40)
        })));
    }
}
