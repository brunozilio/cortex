use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

fn walk(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    paths.sort();
    for path in paths {
        let hidden = path.file_name().is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            walk(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("skills");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    walk(&root, &mut files);
    let mut hasher = DefaultHasher::new();
    let mut entries = String::new();
    for file in &files {
        let Ok(relative) = file.strip_prefix(&root) else { continue };
        let relative = relative.to_string_lossy().replace('\\', "/");
        relative.hash(&mut hasher);
        std::fs::read(file).unwrap_or_default().hash(&mut hasher);
        println!("cargo:rerun-if-changed={}", file.display());
        entries.push_str(&format!("    ({relative:?}, include_bytes!({:?})),\n", file.display().to_string()));
    }
    let code = format!(
        "pub(crate) const FILES: &[(&str, &[u8])] = &[\n{entries}];\n\
         pub(crate) const HASH: u64 = {};\n",
        hasher.finish()
    );
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("builtin_skills.rs");
    if let Err(error) = std::fs::write(&out, code) {
        println!("cargo:warning=could not write the built-in skills: {error}");
    }
}
