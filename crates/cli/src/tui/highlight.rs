//! Code in a diff coloured by its language (from the file's extension), on
//! the line's green or red.

use std::sync::OnceLock;

use ratatui::style::{Color, Style};
use ratatui::text::Span;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let mut themes = ThemeSet::load_defaults().themes;
        themes.remove("base16-ocean.dark").unwrap_or_default()
    })
}

/// The extension of the file a diff heading names (`src/a.rs · Added…`).
pub fn extension(heading: &str) -> Option<&str> {
    let path = heading.split(" · ").next()?.trim();
    let name = path.rsplit('/').next()?;
    name.rsplit_once('.').map(|(_, ext)| ext).filter(|ext| !ext.is_empty())
}

/// `text` as coloured spans for a file with extension `ext`, each on
/// `base` (the line's background); one plain span when the language is
/// unknown.
pub fn spans(ext: Option<&str>, text: &str, base: Style) -> Vec<Span<'static>> {
    let syntax = ext.and_then(|ext| syntaxes().find_syntax_by_extension(ext));
    let Some(syntax) = syntax else { return vec![Span::styled(text.to_owned(), base)] };
    let mut lines = HighlightLines::new(syntax, theme());
    match lines.highlight_line(text, syntaxes()) {
        Ok(pieces) => pieces
            .into_iter()
            .map(|(style, piece)| {
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                Span::styled(piece.to_owned(), base.fg(fg))
            })
            .collect(),
        Err(_) => vec![Span::styled(text.to_owned(), base)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_is_split_into_coloured_pieces_on_the_line_s_background() {
        assert_eq!(extension("src/main.rs · Added 1 lines, removed 0"), Some("rs"));
        assert_eq!(extension("Makefile · Wrote 3 lines"), None);
        let base = Style::new().bg(Color::Indexed(22));
        let pieces = spans(Some("rs"), "let x = 1;", base);
        assert!(pieces.len() > 1, "{pieces:?}");
        assert!(pieces.iter().all(|span| span.style.bg == Some(Color::Indexed(22))));
        assert_eq!(pieces.iter().map(|span| span.content.as_ref()).collect::<String>(), "let x = 1;");
        assert_eq!(spans(None, "plain", base).len(), 1);
    }
}
