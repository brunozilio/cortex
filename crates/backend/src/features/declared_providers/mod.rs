use std::path::PathBuf;

use domain::Principal;
use serde_json::{Value, json};

pub fn handle(_client: u64, _principal: &Principal, input: Value) -> Result<Value, String> {
    let cwd: PathBuf = input["path"].as_str().map(PathBuf::from).ok_or("declared providers need a path")?;
    Ok(json!(crate::native::declare_providers(std::env::home_dir().as_deref(), &cwd)))
}

pub fn forget(_client: u64) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_without_providers_lists_none_and_a_missing_path_is_refused() {
        let empty = tempfile::tempdir().expect("dir");
        let listed = handle(1, &Principal::LocalOwner, json!({ "path": empty.path() })).expect("listed");
        assert!(listed.as_array().is_some_and(|lines| {
            lines.iter().all(|line| !line.to_string().contains(&empty.path().display().to_string()))
        }));
        assert!(handle(1, &Principal::LocalOwner, json!({})).is_err());
    }
}
