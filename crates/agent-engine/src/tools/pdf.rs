use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use platform::bounded::{Bounds, Ended};

const SHOWN: usize = 100_000;

pub fn range(pages: &str) -> Result<(u32, u32), String> {
    let invalid = || format!("pages must be a page or a range like \"1-5\", not {pages:?}");
    let (first, last) = pages.split_once('-').unwrap_or((pages, pages));
    let first: u32 = first.trim().parse().map_err(|_| invalid())?;
    let last: u32 = last.trim().parse().map_err(|_| invalid())?;
    if first == 0 || last < first || last - first >= 50 {
        return Err(format!("{} (at most 50 pages at a time)", invalid()));
    }
    Ok((first, last))
}

pub fn text(path: &Path, pages: Option<&str>, stop: &AtomicBool) -> Result<String, String> {
    let mut command = platform::command("pdftotext");
    command.arg("-layout");
    if let Some(pages) = pages {
        let (first, last) = range(pages)?;
        command.args(["-f", &first.to_string(), "-l", &last.to_string()]);
    }
    command.arg(path).arg("-");
    let bounds = Bounds { timeout: Duration::from_secs(60), max_output: 8 << 20, stop, input: None };
    let output = platform::bounded::run(&mut command, bounds).map_err(|_| {
        "Reading a PDF needs `pdftotext` (poppler: `brew install poppler`, `apt install poppler-utils`).".to_owned()
    })?;
    match output.ended {
        Ended::Exited(status) if status.success() => {}
        Ended::Exited(_) => return Err(format!("pdftotext could not read it: {}", output.stderr.trim())),
        Ended::Stopped => return Err("Stopped reading the PDF.".into()),
        Ended::TimedOut => return Err("pdftotext took over a minute; try fewer pages.".into()),
    }
    let text = output.stdout;
    let pages: Vec<&str> = text.split('\u{c}').collect();
    let first = pages.len().min(1);
    let mut out = String::new();
    for (at, page) in pages.iter().enumerate().filter(|(_, page)| !page.trim().is_empty()) {
        out.push_str(&format!("--- page {} ---\n{}\n", at + first, page.trim_end()));
    }
    Ok(super::clip(&out, SHOWN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_are_one_or_a_short_range() {
        assert_eq!(range("3"), Ok((3, 3)));
        assert_eq!(range("1-5"), Ok((1, 5)));
        assert!(range("0").is_err());
        assert!(range("5-1").is_err());
        assert!(range("1-90").is_err());
        assert!(range("first").is_err());
    }
}
