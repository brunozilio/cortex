//! The little Markdown a reply line needs on a terminal: `**bold**`,
//! `` `code` ``, `# headings` and `- bullets`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// One line of a reply (outside a code block) as styled spans.
pub fn line(text: &str) -> Vec<Span<'static>> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('#') {
        let heading = trimmed.trim_start_matches('#').trim().to_owned();
        return vec![Span::styled(heading, Style::new().add_modifier(Modifier::BOLD))];
    }
    let indent = &text[..text.len() - trimmed.len()];
    let (bullet, rest) = match trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        Some(rest) => (format!("{indent}• "), rest),
        None => (indent.to_owned(), trimmed),
    };
    let mut spans = Vec::new();
    if !bullet.is_empty() {
        spans.push(Span::raw(bullet));
    }
    spans.extend(inline(rest));
    spans
}

fn inline(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let code = rest.find('`');
        let bold = rest.find("**");
        let (at, marker, style) = match (code, bold) {
            (Some(c), Some(b)) if b < c => (b, "**", Style::new().add_modifier(Modifier::BOLD)),
            (_, Some(b)) if code.is_none() => (b, "**", Style::new().add_modifier(Modifier::BOLD)),
            (Some(c), _) => (c, "`", Style::new().fg(Color::Cyan)),
            _ => break,
        };
        let after = &rest[at + marker.len()..];
        let Some(end) = after.find(marker) else { break };
        if at > 0 {
            spans.push(Span::raw(rest[..at].to_owned()));
        }
        spans.push(Span::styled(after[..end].to_owned(), style));
        rest = &after[end + marker.len()..];
    }
    if !rest.is_empty() {
        spans.push(Span::raw(rest.to_owned()));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> Vec<(String, bool, bool)> {
        spans
            .iter()
            .map(|s| {
                let bold = s.style.add_modifier.contains(Modifier::BOLD);
                (s.content.to_string(), bold, s.style.fg == Some(Color::Cyan))
            })
            .collect()
    }

    #[test]
    fn bold_code_bullets_and_headings_are_styled() {
        assert_eq!(
            text(&line("Run **all** tests with `cargo test` now")),
            [
                ("Run ".into(), false, false),
                ("all".into(), true, false),
                (" tests with ".into(), false, false),
                ("cargo test".into(), false, true),
                (" now".into(), false, false),
            ]
        );
        assert_eq!(text(&line("  - item")), [("  • ".into(), false, false), ("item".into(), false, false)]);
        assert_eq!(text(&line("## Plan")), [("Plan".into(), true, false)]);
        assert_eq!(text(&line("a `lone backtick")), [("a `lone backtick".into(), false, false)], "unclosed stays");
    }
}
