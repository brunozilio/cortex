use std::path::{Path, PathBuf};
use std::process::Command;

use domain::Principal;
use serde_json::{Value, json};

pub fn handle(_client: u64, _principal: &Principal, input: Value) -> Result<Value, String> {
    let home = std::env::home_dir().ok_or("no home folder to keep plugins in")?;
    let folder = home.join(brand::DIR).join("plugins");
    let text = |key: &str| input[key].as_str().map(str::trim).filter(|value| !value.is_empty());
    match input["op"].as_str().unwrap_or("list") {
        "list" => Ok(json!(list(&folder))),
        "install" => install(&folder, text("source").ok_or("install needs a git URL or path")?, text("name")),
        "update" => update(&folder, text("name").ok_or("update needs a plugin's name")?),
        "remove" => remove(&folder, text("name").ok_or("remove needs a plugin's name")?),
        other => Err(format!("the plugins feature has no {other:?}")),
    }
}

pub fn forget(_client: u64) {}

fn list(folder: &Path) -> Vec<Value> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).filter(|p| p.is_dir()).collect();
    found.sort();
    found
        .iter()
        .map(|root| {
            let manifest = std::fs::read_to_string(root.join(concat!(".", brand::command!(), "-plugin/plugin.json")))
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or_default();
            let name = root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            json!({ "name": name, "description": manifest["description"].as_str().unwrap_or_default() })
        })
        .collect()
}

fn install(folder: &Path, source: &str, name: Option<&str>) -> Result<Value, String> {
    let name = match name {
        Some(name) => name.to_owned(),
        None => named_after(source).ok_or_else(|| format!("cannot tell a name from {source}; pass --name"))?,
    };
    let target = plugin(folder, &name)?;
    if target.exists() {
        return Err(format!("{name} is already installed; update or remove it"));
    }
    std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    git(folder, &["clone", "--depth", "1", "--", source, &name])?;
    Ok(json!({ "installed": name, "at": target }))
}

fn update(folder: &Path, name: &str) -> Result<Value, String> {
    let target = plugin(folder, name)?;
    if !target.join(".git").exists() {
        return Err(format!("{name} is not a git checkout here, so it cannot be updated"));
    }
    let pulled = git(&target, &["pull", "--ff-only"])?;
    Ok(json!({ "updated": name, "git": pulled.trim() }))
}

fn remove(folder: &Path, name: &str) -> Result<Value, String> {
    let target = plugin(folder, name)?;
    if !target.is_dir() {
        return Err(format!("no plugin called {name}"));
    }
    std::fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    Ok(json!({ "removed": name }))
}

fn plugin(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let plain = name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) && !name.starts_with('.');
    plain.then(|| folder.join(name)).ok_or_else(|| format!("{name:?} is not a plugin name"))
}

fn named_after(source: &str) -> Option<String> {
    let last = source.trim_end_matches('/').rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty()).then(|| name.to_owned())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git").args(args).current_dir(cwd).output().map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
