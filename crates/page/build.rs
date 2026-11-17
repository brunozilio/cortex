use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

const SOURCES: [&str; 9] = [
    "src",
    "docs",
    "public",
    "index.html",
    "package.json",
    "bun.lock",
    "vite.config.ts",
    "tailwind.config.js",
    "tsconfig.app.json",
];
const SKIP: &str = "SKIP_WEB_BUILD";

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let web = manifest.join("../../web");
    let dist = web.join("dist");
    println!("cargo::rerun-if-env-changed={SKIP}");
    println!("cargo::rerun-if-changed={}", dist.display());
    for source in SOURCES {
        println!("cargo::rerun-if-changed={}", web.join(source).display());
    }
    if std::env::var_os(SKIP).is_none() && web.join("package.json").is_file() && stale(&web) {
        rebuild(&web)?;
    }
    let files = if dist.join("index.html").is_file() { listed(&dist, &dist)? } else { Vec::new() };
    std::fs::write(out.join("bundle.rs"), source(&files))?;
    Ok(())
}

fn stale(web: &Path) -> bool {
    let Ok(built) = std::fs::metadata(web.join("dist/index.html")).and_then(|found| found.modified()) else {
        return true;
    };
    SOURCES.iter().filter_map(|source| newest(&web.join(source))).any(|changed| changed > built)
}

fn newest(path: &Path) -> Option<SystemTime> {
    let found = std::fs::metadata(path).ok()?;
    if !found.is_dir() {
        return found.modified().ok();
    }
    std::fs::read_dir(path).ok()?.flatten().filter_map(|entry| newest(&entry.path())).max()
}

fn rebuild(web: &Path) -> Result<(), Box<dyn Error>> {
    if Command::new("bun").arg("--version").output().is_err() {
        println!(
            "cargo::warning=web/ changed but Bun is not installed, so the page embedded is the last one built; \
             install Bun from https://bun.sh or run `cargo run -p xtask -- web`"
        );
        return Ok(());
    }
    let lock = std::fs::File::create(web.join(".build.lock"))?;
    lock.lock()?;
    if stale(web) {
        run(web, &["install", "--frozen-lockfile"])?;
        run(web, &["run", "build"])?;
    }
    lock.unlock()?;
    Ok(())
}

fn run(web: &Path, arguments: &[&str]) -> Result<(), Box<dyn Error>> {
    let done = Command::new("bun").args(arguments).current_dir(web).output()?;
    if done.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&done.stderr);
    let printed = String::from_utf8_lossy(&done.stdout);
    Err(format!("`bun {}` failed in {} ({}):\n{printed}\n{said}", arguments.join(" "), web.display(), done.status)
        .into())
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
