use std::path::{Path, PathBuf};

use crate::api::features::plugins::{Plugin, PluginsReply, PluginsRequest};
use serde_json::Value;

use crate::features::context::Context;

pub fn handle(context: &Context<'_>, request: PluginsRequest) -> Result<PluginsReply, String> {
    let home = context.services.home().ok_or("no home folder to keep plugins in")?;
    let folder = home.join(brand::DIR).join("plugins");
    match request {
        PluginsRequest::List => Ok(PluginsReply::Listed(list(&folder))),
        PluginsRequest::Install { source, name } => {
            let source = said(&source).ok_or("install needs a git URL or path")?;
            install(&folder, source, name.as_deref().and_then(said))
        }
        PluginsRequest::Update { name } => update(&folder, said(&name).ok_or("update needs a plugin's name")?),
        PluginsRequest::Remove { name } => remove(&folder, said(&name).ok_or("remove needs a plugin's name")?),
    }
}

fn said(text: &str) -> Option<&str> {
    Some(text.trim()).filter(|text| !text.is_empty())
}

pub fn forget(_client: u64) {}

fn list(folder: &Path) -> Vec<Plugin> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.is_dir())
        .collect();
    found.sort();
    found
        .iter()
        .map(|root| {
            let manifest = std::fs::read_to_string(root.join(concat!(".", brand::command!(), "-plugin/plugin.json")))
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or_default();
            let name = root.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            Plugin { name, description: manifest["description"].as_str().unwrap_or_default().to_owned() }
        })
        .collect()
}

fn install(folder: &Path, source: &str, name: Option<&str>) -> Result<PluginsReply, String> {
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
    Ok(PluginsReply::Installed { name, at: target })
}

fn update(folder: &Path, name: &str) -> Result<PluginsReply, String> {
    let target = plugin(folder, name)?;
    if !target.join(".git").exists() {
        return Err(format!("{name} is not a git checkout here, so it cannot be updated"));
    }
    let pulled = git(&target, &["pull", "--ff-only"])?;
    Ok(PluginsReply::Updated { name: name.to_owned(), git: pulled.trim().to_owned() })
}

fn remove(folder: &Path, name: &str) -> Result<PluginsReply, String> {
    let target = plugin(folder, name)?;
    if !target.is_dir() {
        return Err(format!("no plugin called {name}"));
    }
    std::fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    Ok(PluginsReply::Removed { name: name.to_owned() })
}

fn plugin(folder: &Path, name: &str) -> Result<PathBuf, String> {
    let plain =
        name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')) && !name.starts_with('.');
    plain.then(|| folder.join(name)).ok_or_else(|| format!("{name:?} is not a plugin name"))
}

fn named_after(source: &str) -> Option<String> {
    let last = source.trim_end_matches('/').rsplit(['/', ':']).next()?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    (!name.is_empty()).then(|| name.to_owned())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    crate::git::run(cwd, args, crate::git::REMOTE)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
