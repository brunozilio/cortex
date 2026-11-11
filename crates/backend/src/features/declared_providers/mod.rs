use std::path::PathBuf;

use serde_json::{Value, json};

use crate::feature::Context;

pub fn handle(_context: &Context<'_>, input: Value) -> Result<Value, String> {
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
        crate::feature::testing::local(1, |context| {
            let listed = handle(context, json!({ "path": empty.path() })).expect("listed");
            assert!(listed.as_array().is_some_and(|lines| {
                lines.iter().all(|line| !line.to_string().contains(&empty.path().display().to_string()))
            }));
            assert!(handle(context, json!({})).is_err());
        });
    }
}
