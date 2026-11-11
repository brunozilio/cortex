//! The release notes `/release-notes` shows: the newest CHANGELOG.md
//! section, without its links (they name the repository).
//!
//! macOS: ONNX Runtime's CoreML code (memory embeddings, via fastembed)
//! calls `___isPlatformVersionAtLeast`, which lives in clang's runtime
//! library; rustc's link line does not include it.

use std::path::PathBuf;
use std::process::Command;

/// `[text](target)` as `text`, and ` ([text](target))` references dropped.
fn unlinked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find("](").map(|at| open + at) else { break };
        let Some(end) = rest[close..].find(')').map(|at| close + at) else { break };
        let label = &rest[open + 1..close];
        let before = &rest[..open];
        // A reference in parentheses, `([#16](…))`, goes entirely.
        if before.ends_with('(') && rest[end + 1..].starts_with(')') {
            out.push_str(before.trim_end_matches('(').trim_end());
            rest = &rest[end + 2..];
        } else {
            out.push_str(before);
            out.push_str(label);
            rest = &rest[end + 1..];
        }
    }
    out.push_str(rest);
    out
}

fn release_notes() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let changelog = manifest.join("../../CHANGELOG.md");
    println!("cargo:rerun-if-changed={}", changelog.display());
    let text = std::fs::read_to_string(&changelog).unwrap_or_default();
    let latest = text.split("\n## ").nth(1).map(|section| format!("## {}", section.trim())).unwrap_or_default();
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("release_notes.md");
    if let Err(error) = std::fs::write(&out, unlinked(&latest)) {
        println!("cargo:warning=could not write the release notes: {error}");
    }
}

fn main() {
    release_notes();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let Ok(output) = Command::new("clang").arg("--print-file-name=libclang_rt.osx.a").output() else { return };
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if path.is_absolute() && path.exists() {
        println!("cargo:rustc-link-arg={}", path.display());
    }
}
