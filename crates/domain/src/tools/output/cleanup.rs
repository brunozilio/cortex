pub const PLAIN_ENV: [(&str, &str); 5] = [
    ("NO_COLOR", "1"),
    ("CLICOLOR", "0"),
    ("CARGO_TERM_COLOR", "never"),
    ("CARGO_TERM_PROGRESS_WHEN", "never"),
    ("PY_COLORS", "0"),
];

const REPEATS_SHOWN_ONCE: usize = 3;

pub fn clean(output: &str) -> String {
    let plain = without_escapes(output);
    let lines: Vec<&str> = plain.split('\n').map(last_redraw).collect();
    collapsed(&lines)
}

fn without_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                Some('(' | ')' | '*' | '+') => {
                    chars.next();
                }
                _ => {}
            },
            '\u{8}' => {
                out.pop();
            }
            c => out.push(c),
        }
    }
    out
}

fn last_redraw(line: &str) -> &str {
    line.trim_end_matches('\r').rsplit('\r').next().unwrap_or_default()
}

fn collapsed(lines: &[&str]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let run = lines[i..].iter().take_while(|next| **next == line).count();
        if line.trim().is_empty() {
            if out.last().is_none_or(|last| !last.trim().is_empty()) {
                out.push(String::new());
            }
        } else if run >= REPEATS_SHOWN_ONCE {
            out.push(format!("{line} (×{run})"));
        } else {
            out.extend(std::iter::repeat_n(line.to_owned(), run));
        }
        i += run;
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_terminal_titles_are_dropped() {
        assert_eq!(clean("\u{1b}[1;31merror\u{1b}[0m: bad"), "error: bad");
        assert_eq!(clean("\u{1b}]0;title\u{7}ok \u{1b}]8;;http://x\u{1b}\\link\u{1b}]8;;\u{1b}\\"), "ok link");
        assert_eq!(clean("\u{1b}(Bplain"), "plain");
    }

    #[test]
    fn a_redrawn_line_keeps_only_what_the_terminal_showed_last() {
        assert_eq!(clean("Downloading 10%\rDownloading 50%\rDownloading 100%\r\ndone\r\n"), "Downloading 100%\ndone\n");
        assert_eq!(clean("spin|\u{8}/\u{8}-"), "spin-");
    }

    #[test]
    fn repeated_lines_are_counted_and_blank_runs_kept_once() {
        assert_eq!(clean("warn: x\nwarn: x\nwarn: x\nwarn: x\nend"), "warn: x (×4)\nend");
        assert_eq!(clean("a\na\nb"), "a\na\nb");
        assert_eq!(clean("a\n\n\n\nb"), "a\n\nb");
    }
}
