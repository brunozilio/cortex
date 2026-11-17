use std::error::Error;
use std::path::{Path, PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let web = manifest.join("../../web");
    let dist = web.join("dist");
    let built = dist.join("index.html").is_file();
    let watched = if dist.is_dir() { &dist } else { &web };
    println!("cargo::rerun-if-changed={}", watched.display());
    let files = if built { listed(&dist, &dist)? } else { Vec::new() };
    std::fs::write(out.join("bundle.rs"), source(&files))?;
    Ok(())
}

fn listed(root: &Path, folder: &Path) -> Result<Vec<(String, PathBuf)>, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let path = entry?.path();
        if path.is_dir() {
            files.extend(listed(root, &path)?);
        } else if path.extension().is_none_or(|extension| extension != "map") {
            let relative = path.strip_prefix(root)?.components();
            let served = relative.map(|part| part.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/");
            files.push((served, path.canonicalize()?));
        }
    }
    files.sort();
    Ok(files)
}

fn source(files: &[(String, PathBuf)]) -> String {
    let entries: String = files
        .iter()
        .map(|(served, path)| {
            format!("    Asset {{ path: {served:?}, bytes: include_bytes!({:?}) }},\n", path.display().to_string())
        })
        .collect();
    format!("pub static BUNDLE: &[Asset] = &[\n{entries}];\n")
}
