use std::path::{Path, PathBuf};

use domain::{CustomAgent, ModelOption, PromptRequest, Skill};

#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub plugins: Vec<PathBuf>,
}

impl Extensions {
    fn for_run(&self) -> domain::Extensions {
        domain::Extensions { plugins: self.plugins.clone(), ..Default::default() }
    }

    pub(crate) fn brief(&self, request: PromptRequest, home: Option<PathBuf>) -> PromptRequest {
        let extensions = domain::Extensions { home, ..self.for_run() };
        PromptRequest { extensions: (!extensions.is_empty()).then_some(extensions), ..request }
    }
}

fn session(cwd: &Path, extensions: &Extensions) -> PromptRequest {
    let mut request = PromptRequest::plain(cwd.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(extensions.for_run());
    request
}

pub(crate) fn skills(cwd: &Path, extensions: &Extensions, home: Option<&Path>) -> Vec<Skill> {
    agent_engine::tools::skill::discover(&session(cwd, extensions), home)
        .into_iter()
        .map(|found| Skill { name: found.name, description: found.description })
        .collect()
}

pub(crate) fn agents(cwd: &Path, extensions: &Extensions, home: Option<&Path>) -> Vec<CustomAgent> {
    agent_engine::tools::subagent::discover(&session(cwd, extensions), home)
        .into_iter()
        .map(|found| CustomAgent { name: found.name, description: found.description })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_installed_plugin_s_commands_are_offered_under_its_name() {
        let dir = tempfile::tempdir().expect("dir");
        let plugin = dir.path().join("acme");
        std::fs::create_dir_all(plugin.join("commands")).expect("commands");
        std::fs::write(plugin.join("commands/ship.md"), "---\ndescription: Ships\n---\nShip it.").expect("command");
        let extensions = Extensions { plugins: vec![plugin] };
        let names: Vec<String> = skills(dir.path(), &extensions, None).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"acme:ship".into()), "{names:?}");
    }

    #[test]
    fn a_run_gets_the_host_s_plugins() {
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        assert_eq!(Extensions::default().brief(request.clone(), None), request, "nothing to add");
        let extensions = Extensions { plugins: vec!["/plugins/acme".into()] };
        let added = extensions.brief(request, None).extensions.expect("extensions");
        assert_eq!(added.plugins, [PathBuf::from("/plugins/acme")]);
    }

    #[test]
    fn a_run_gets_the_home_its_owner_was_given() {
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        let briefed = Extensions::default().brief(request, Some("/users/ana".into()));
        assert_eq!(briefed.extensions.and_then(|extensions| extensions.home), Some(PathBuf::from("/users/ana")));
    }

    #[test]
    fn listings_read_the_given_home_not_the_machine_s() {
        let dir = tempfile::tempdir().expect("dir");
        let home = dir.path().join("ana");
        std::fs::create_dir_all(home.join(brand::DIR).join("agents")).expect("agents");
        std::fs::write(
            home.join(brand::DIR).join("agents/helper.md"),
            "---\nname: helper\ndescription: Helps\n---\nHelp.",
        )
        .expect("agent");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).expect("project");
        let named =
            |home: Option<&Path>| agents(&project, &Extensions::default(), home).iter().any(|a| a.name == "helper");
        assert!(named(Some(&home)));
        assert!(!named(None));
    }
}
