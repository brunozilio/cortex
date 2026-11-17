use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;

const SKIP: &str = "CONSOLE_SKIP_WEB";
const UNWATCHED: [&str; 4] = ["node_modules", "dist", "test", "testing"];

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out = PathBuf::from(std::env::var("OUT_DIR")?);
    let web = manifest.join("../../web");
    let dist = out.join("web");
    println!("cargo::rerun-if-env-changed={SKIP}");
    watch(&web);
    let files = match build(&web, &dist) {
        Ok(()) => listed(&dist, &dist)?,
        Err(reason) => {
            println!("cargo::warning=the console app is not embedded: {reason}");
            Vec::new()
        }
    };
    std::fs::write(out.join("bundle.rs"), source(&files))?;
    Ok(())
}

fn watch(folder: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        println!("cargo::rerun-if-changed={}", folder.display());
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if UNWATCHED.contains(&name.as_ref()) || name.starts_with('.') {
            continue;
        }
        println!("cargo::rerun-if-changed={}", entry.path().display());
    }
}

fn build(web: &Path, dist: &Path) -> Result<(), String> {
    if std::env::var_os(SKIP).is_some() {
        return Err(format!("{SKIP} is set"));
    }
    if !web.join("package.json").is_file() {
        return Err(format!("{} has no package.json", web.display()));
    }
    if !web.join("node_modules").is_dir() {
        bun(web, &["install", "--frozen-lockfile"])?;
    }
    let target = dist.to_str().ok_or("the build folder's path is not UTF-8")?;
    bun(web, &["run", "build", "--outDir", target, "--emptyOutDir"])
}

fn bun(web: &Path, arguments: &[&str]) -> Result<(), String> {
    let output = Command::new("bun")
        .args(arguments)
        .current_dir(web)
        .output()
        .map_err(|error| format!("could not run bun ({error}); install it from https://bun.sh"))?;
    if output.status.success() {
        return Ok(());
    }
    let printed = [output.stderr, output.stdout].concat();
    let printed = String::from_utf8_lossy(&printed);
    let reason = printed.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("no output");
    Err(format!("`bun {}` failed: {reason}", arguments.join(" ")))
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
            files.push((served, path));
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
