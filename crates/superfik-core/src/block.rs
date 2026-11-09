//! A reply's text split into the blocks it is drawn as.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    Paragraph(String),
    Heading(String),
    Code(String),
    Bullet(String),
    Table(Vec<String>, Vec<Vec<String>>),
    Numbered(String, String),
    Visual { svg: String, complete: bool },
}

fn numbered(line: &str) -> Option<(&str, &str)> {
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    let rest = line.get(digits..)?;
    let after = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") "))?;
    (digits > 0 && digits <= 3).then(|| (&line[..digits + 1], after))
}

fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(|c| c.trim().to_owned()).collect()
}

fn is_separator(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('|') && line.contains('-') && line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

impl Block {
    pub fn parse(text: &str) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut paragraph = String::new();
        let flush = |paragraph: &mut String, blocks: &mut Vec<Block>| {
            if !paragraph.trim().is_empty() {
                blocks.push(Block::Paragraph(paragraph.trim().to_owned()));
            }
            paragraph.clear();
        };
        let mut fence: Option<(bool, Vec<&str>)> = None;
        let fenced = |visual: bool, lines: &[&str], complete: bool| {
            let body = lines.join("\n").trim_matches('\n').to_owned();
            if visual { Block::Visual { svg: body, complete } } else { Block::Code(body) }
        };
        let lines: Vec<&str> = text.lines().collect();
        let mut next = 0;
        while let Some(&line) = lines.get(next) {
            next += 1;
            let trimmed = line.trim_start();
            if let Some((visual, lines)) = fence.as_mut() {
                if trimmed.starts_with("```") {
                    blocks.push(fenced(*visual, lines, true));
                    fence = None;
                } else {
                    lines.push(line);
                }
                continue;
            }
            if let Some(info) = trimmed.strip_prefix("```") {
                flush(&mut paragraph, &mut blocks);
                fence = Some((info.trim() == crate::visual::FENCE, Vec::new()));
                continue;
            }
            if trimmed.starts_with('|') && lines.get(next).is_some_and(|l| is_separator(l)) {
                flush(&mut paragraph, &mut blocks);
                let header = cells(trimmed);
                let mut rows = Vec::new();
                next += 1;
                while let Some(row) = lines.get(next).filter(|l| l.trim_start().starts_with('|')) {
                    let mut row = cells(row);
                    row.resize(header.len(), String::new());
                    rows.push(row);
                    next += 1;
                }
                blocks.push(Block::Table(header, rows));
                continue;
            }
            let heading = trimmed.trim_start_matches('#');
            if heading.len() < trimmed.len() && heading.starts_with(' ') {
                flush(&mut paragraph, &mut blocks);
                blocks.push(Block::Heading(heading.trim().to_owned()));
                continue;
            }
            if let Some(item) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
                flush(&mut paragraph, &mut blocks);
                blocks.push(Block::Bullet(item.trim().to_owned()));
            } else if let Some((marker, item)) = numbered(trimmed) {
                flush(&mut paragraph, &mut blocks);
                blocks.push(Block::Numbered(marker.to_owned(), item.trim().to_owned()));
            } else if trimmed.is_empty() {
                flush(&mut paragraph, &mut blocks);
            } else {
                if !paragraph.is_empty() {
                    paragraph.push(' ');
                }
                paragraph.push_str(trimmed);
            }
        }
        flush(&mut paragraph, &mut blocks);
        if let Some((visual, lines)) = fence.filter(|(visual, lines)| *visual || !lines.is_empty()) {
            blocks.push(fenced(visual, &lines, false));
        }
        blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_are_their_own_block() {
        let text = "## Plan\nFirst line\nsecond";
        assert_eq!(
            Block::parse(text),
            vec![Block::Heading("Plan".into()), Block::Paragraph("First line second".into())]
        );
    }

    #[test]
    fn fenced_code_keeps_its_lines_and_markdown_untouched() {
        let text = "Run:\n```sh\ncargo test\n\n- not a bullet\n```\nDone.";
        assert_eq!(
            Block::parse(text),
            vec![
                Block::Paragraph("Run:".into()),
                Block::Code("cargo test\n\n- not a bullet".into()),
                Block::Paragraph("Done.".into()),
            ]
        );
    }

    #[test]
    fn a_fence_still_streaming_shows_what_arrived() {
        assert_eq!(Block::parse("```\nlet x = 1;"), vec![Block::Code("let x = 1;".into())]);
        assert_eq!(Block::parse("```"), Vec::<Block>::new());
    }

    #[test]
    fn visual_fences_hold_a_drawing_complete_once_closed() {
        let svg = "<svg viewBox=\"0 0 440 80\">\n<rect width=\"4\" height=\"4\"/>\n</svg>";
        let text = format!("Here:\n```visual\n{svg}\n```\nAfter.");
        assert_eq!(
            Block::parse(&text),
            vec![
                Block::Paragraph("Here:".into()),
                Block::Visual { svg: svg.into(), complete: true },
                Block::Paragraph("After.".into()),
            ]
        );
        assert_eq!(
            Block::parse("```visual\n<svg viewBox"),
            vec![Block::Visual { svg: "<svg viewBox".into(), complete: false }]
        );
        assert_eq!(Block::parse("```visual"), vec![Block::Visual { svg: String::new(), complete: false }]);
        assert_eq!(Block::parse("```svg\n<svg/>\n```"), vec![Block::Code("<svg/>".into())], "svg is code to read");
    }

    #[test]
    fn pipe_tables_have_a_header_and_padded_rows() {
        let text = "Plan:\n| Step | Cost |\n|---|:-:|\n| Build | 2 |\n| Ship |\nAfter.";
        assert_eq!(
            Block::parse(text),
            vec![
                Block::Paragraph("Plan:".into()),
                Block::Table(
                    vec!["Step".into(), "Cost".into()],
                    vec![vec!["Build".into(), "2".into()], vec!["Ship".into(), String::new()]],
                ),
                Block::Paragraph("After.".into()),
            ]
        );
    }

    #[test]
    fn a_pipe_line_without_a_separator_is_text() {
        assert_eq!(Block::parse("| a | b |"), vec![Block::Paragraph("| a | b |".into())]);
    }

    #[test]
    fn hash_without_a_space_is_text() {
        assert_eq!(Block::parse("#tag"), vec![Block::Paragraph("#tag".into())]);
    }

    #[test]
    fn parses_numbered_items() {
        let text = "Fruits:\n1. Apple\n2) Banana\n2026 was great.";
        assert_eq!(
            Block::parse(text),
            vec![
                Block::Paragraph("Fruits:".into()),
                Block::Numbered("1.".into(), "Apple".into()),
                Block::Numbered("2)".into(), "Banana".into()),
                Block::Paragraph("2026 was great.".into()),
            ]
        );
    }

    #[test]
    fn parses_paragraphs_and_bullets() {
        let text = "Here's a sky.\n\n- One gradient\n- A moon\n  that glows\nWant me to build it?";
        assert_eq!(
            Block::parse(text),
            vec![
                Block::Paragraph("Here's a sky.".into()),
                Block::Bullet("One gradient".into()),
                Block::Bullet("A moon".into()),
                Block::Paragraph("that glows Want me to build it?".into()),
            ]
        );
    }
}
