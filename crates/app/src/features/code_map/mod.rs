pub mod run;

use std::path::{Path, PathBuf};

use crate::api::features::code_map::{CodeMapReply, CodeMapRequest};

use crate::features::context::Context;

pub const MOST_FILES: usize = 12;

pub fn handle(context: &Context<'_>, request: CodeMapRequest) -> Result<CodeMapReply, String> {
    let CodeMapRequest { path, prompt } = request;
    let map = pointers(context, &path, &prompt);
    Ok(CodeMapReply { path, map })
}

pub fn forget(_client: u64) {}

pub fn pointers(context: &Context<'_>, project: &Path, prompt: &str) -> Option<String> {
    let home = context.services.home();
    let on = context
        .services
        .project_settings(project)
        .is_ok_and(|settings| settings.features.on(domain::features::CODE_MAP));
    if !on || !mapped(project, home.as_deref()) {
        return None;
    }
    let map = code_map::refresh(project, &cache(context, project));
    let found = map.relevant(prompt, MOST_FILES);
    let shown = code_map::render(&found);
    (!shown.trim().is_empty()).then_some(shown)
}

fn mapped(project: &Path, home: Option<&Path>) -> bool {
    home != Some(project) && project.ancestors().any(|folder| folder.join(".git").exists())
}

fn cache(context: &Context<'_>, project: &Path) -> PathBuf {
    context.services.root.code_map().join(format!("{}.json", crate::dirs::project_key(project)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::context::testing;

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::create_dir(dir.path().join(".git")).expect("git");
        std::fs::create_dir(dir.path().join("src")).expect("src");
        std::fs::write(
            dir.path().join("src/retry.rs"),
            "pub fn merge_near_duplicates() {}\nfn caller() { merge_near_duplicates(); }\n",
        )
        .expect("write");
        dir
    }

    #[test]
    fn a_prompt_naming_a_function_points_at_the_file_that_defines_it() {
        let dir = project();
        let data = tempfile::tempdir().expect("data");
        let services = testing::services(Some(data.path()));

        let shown = testing::with(1, &services, |context| {
            pointers(context, dir.path(), "why does merge_near_duplicates drop things?")
        })
        .expect("pointers");

        assert!(shown.contains("<code-map>") && shown.contains("src/retry.rs"), "{shown}");
        assert!(data.path().join("code-map").read_dir().expect("cache").count() == 1);
    }

    #[test]
    fn a_folder_outside_git_or_a_prompt_that_matches_nothing_gets_no_map() {
        let data = tempfile::tempdir().expect("data");
        let services = testing::services(Some(data.path()));
        let plain = tempfile::tempdir().expect("plain");
        let dir = project();

        testing::with(1, &services, |context| {
            assert_eq!(pointers(context, plain.path(), "merge_near_duplicates"), None);
            assert_eq!(pointers(context, dir.path(), "hello there"), None);
            let asked = |prompt: &str| CodeMapRequest { path: dir.path().to_path_buf(), prompt: prompt.into() };
            let none = CodeMapReply { path: dir.path().to_path_buf(), map: None };
            assert_eq!(handle(context, asked("")), Ok(none.clone()), "an empty prompt points nowhere");
            assert_eq!(handle(context, asked("hello there")), Ok(none));
            let found = handle(context, asked("merge_near_duplicates")).expect("answered");
            assert!(found.map.is_some_and(|map| map.contains("src/retry.rs")));
        });
    }

    #[test]
    fn a_project_that_turns_the_code_map_off_is_never_scanned() {
        let dir = project();
        std::fs::create_dir_all(dir.path().join(brand::DIR)).expect("settings folder");
        let off = serde_json::json!({ "features": { domain::features::CODE_MAP: false } });
        std::fs::write(dir.path().join(brand::DIR).join("settings.json"), off.to_string()).expect("settings");
        let data = tempfile::tempdir().expect("data");
        let services = testing::services(Some(data.path()));

        let shown = testing::with(1, &services, |context| pointers(context, dir.path(), "merge_near_duplicates"));

        assert_eq!(shown, None);
        assert!(!data.path().join("code-map").exists());
    }
}
