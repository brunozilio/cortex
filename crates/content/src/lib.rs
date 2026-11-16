use std::path::{Path, PathBuf};

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/content.rs"));
}

mod install;
mod render;

pub use install::{Installed, MANIFEST, install_files, install_once};

pub use render::{LIMIT, call, call_in, exports, render, render_export, render_export_in, render_in, value, value_in};

pub fn brand() -> serde_json::Value {
    serde_json::json!({
        "NAME": brand::NAME, "COMMAND": brand::COMMAND, "DIR": brand::DIR,
        "INSTRUCTIONS": brand::INSTRUCTIONS, "ENV": brand::ENV,
    })
}

pub fn with_brand(data: &serde_json::Value) -> serde_json::Value {
    let mut data = match data {
        serde_json::Value::Object(fields) => serde_json::Value::Object(fields.clone()),
        _ => serde_json::json!({}),
    };
    if data.get("brand").is_none() {
        data["brand"] = brand();
    }
    data
}

pub fn hash() -> u64 {
    embedded::HASH
}

pub fn builtin(path: &str) -> Option<&'static [u8]> {
    embedded::FILES.iter().find(|(file, _)| *file == path).map(|(_, bytes)| *bytes)
}

pub fn builtin_files(prefix: &str) -> Vec<(&'static str, &'static [u8])> {
    embedded::FILES.iter().filter(|(file, _)| file.starts_with(prefix)).copied().collect()
}

static FOLDER: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn use_folder(folder: PathBuf) -> Result<(), PathBuf> {
    FOLDER.set(folder)
}

pub fn folder() -> Option<PathBuf> {
    match FOLDER.get() {
        Some(folder) => Some(folder.clone()),
        None => std::env::home_dir().map(|home| home.join(brand::DIR)),
    }
}

pub fn override_path(folder: &Path, path: &str) -> Option<PathBuf> {
    let relative = Path::new(path);
    let plain = relative.components().all(|part| matches!(part, std::path::Component::Normal(_)));
    plain.then(|| folder.join(relative))
}

pub fn user_text(folder: Option<&Path>, path: &str) -> Option<String> {
    let file = folder.and_then(|folder| override_path(folder, path))?;
    std::fs::read_to_string(file).ok().map(|text| brand::text(text.trim_end_matches('\n')))
}

pub fn source(path: &str) -> Option<String> {
    user_text(folder().as_deref(), path).or_else(|| builtin_text(path))
}

pub fn builtin_text(path: &str) -> Option<String> {
    builtin(path).map(|bytes| brand::text(String::from_utf8_lossy(bytes).trim_end_matches('\n')))
}

pub fn text_in(folder: Option<&Path>, path: &str) -> String {
    let overridden =
        folder.and_then(|folder| override_path(folder, path)).and_then(|file| std::fs::read_to_string(file).ok());
    let raw = overridden
        .unwrap_or_else(|| builtin(path).map(|bytes| String::from_utf8_lossy(bytes).into_owned()).unwrap_or_default());
    brand::text(raw.trim_end_matches('\n'))
}

pub fn text(path: &str) -> String {
    if path.ends_with(".js") {
        return render(path, &with_brand(&serde_json::Value::Null));
    }
    text_in(folder().as_deref(), path)
}

pub fn paths() -> Vec<&'static str> {
    embedded::FILES.iter().map(|(file, _)| *file).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_host_s_folder_replaces_the_home_one_and_is_set_once() {
        let chosen = tempfile::tempdir().expect("folder");
        assert_eq!(use_folder(chosen.path().to_path_buf()), Ok(()));
        assert_eq!(folder().as_deref(), Some(chosen.path()));
        assert!(use_folder("/elsewhere".into()).is_err());
    }

    #[test]
    fn a_user_file_wins_over_the_built_in_one_and_is_read_each_time() {
        let folder = tempfile::tempdir().expect("folder");
        let path = paths().into_iter().find(|path| path.ends_with(".md")).expect("a text");
        let built_in = text_in(None, path);
        assert!(!built_in.is_empty());
        assert_eq!(text_in(Some(folder.path()), path), built_in, "nothing overridden yet");
        let mine = override_path(folder.path(), path).expect("plain");
        std::fs::create_dir_all(mine.parent().expect("parent")).expect("dir");
        std::fs::write(&mine, "Mine, from {{NAME}}.\n").expect("edit");
        assert_eq!(text_in(Some(folder.path()), path), format!("Mine, from {}.", brand::NAME));
        std::fs::write(&mine, "Changed again.").expect("edit");
        assert_eq!(text_in(Some(folder.path()), path), "Changed again.", "read each time");
    }

    #[test]
    fn only_plain_paths_are_read_or_written() {
        let folder = tempfile::tempdir().expect("folder");
        assert!(override_path(folder.path(), "../secrets").is_none());
        assert!(override_path(folder.path(), "/etc/passwd").is_none());
    }
}
