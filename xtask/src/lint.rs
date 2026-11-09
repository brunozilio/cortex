//! `xtask lint`: the CLAUDE.md rules clippy cannot see. Findings already
//! present are recorded in `xtask/lint-baseline.txt` (one `rule path` per
//! line); only findings outside it fail. `lint --update-baseline` rewrites it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const MAX_LINES: usize = 300;
const THEME_DIR: &str = "crates/superfik-ui/src/theme/";
const BASELINE: &str = "xtask/lint-baseline.txt";

pub fn run(root: &Path, update_baseline: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut found = BTreeSet::new();
    let mut files = Vec::new();
    collect(&root.join("crates"), &mut files)?;
    for file in files {
        let rel = file.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
        let source = std::fs::read_to_string(&file)?;
        found.extend(check(&rel, &source));
    }
    let baseline_path = root.join(BASELINE);
    if update_baseline {
        std::fs::write(&baseline_path, found.iter().map(|f| format!("{f}\n")).collect::<String>())?;
        println!("baseline: {} findings", found.len());
        return Ok(());
    }
    let known: BTreeSet<String> =
        std::fs::read_to_string(&baseline_path).unwrap_or_default().lines().map(str::to_owned).collect();
    let new: Vec<_> = found.difference(&known).collect();
    for finding in &new {
        eprintln!("lint: {finding}");
    }
    if new.is_empty() {
        println!("lint: ok ({} baselined)", known.len());
        Ok(())
    } else {
        Err(format!("{} new lint finding(s)", new.len()).into())
    }
}

fn check(rel: &str, source: &str) -> Vec<String> {
    let mut out = Vec::new();
    if source.lines().count() > MAX_LINES {
        out.push(format!("file-too-long {rel}"));
    }
    let production = source.split("#[cfg(test)]").next().unwrap_or(source);
    if !rel.starts_with(THEME_DIR) && production.contains("Color::hex(") {
        out.push(format!("colour-outside-theme {rel}"));
    }
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_long_files() {
        let long = "fn f() {}\n".repeat(MAX_LINES + 1);
        assert_eq!(check("crates/a/src/x.rs", &long), ["file-too-long crates/a/src/x.rs"]);
    }

    #[test]
    fn flags_colours_outside_theme_but_not_in_tests_or_theme() {
        let src = "let c = Color::hex(0xffffff);";
        assert_eq!(check("crates/a/src/x.rs", src).len(), 1);
        assert!(check("crates/superfik-ui/src/theme/colors.rs", src).is_empty());
        assert!(check("crates/a/src/x.rs", "#[cfg(test)]\nmod t { Color::hex(1); }").is_empty());
    }
}
