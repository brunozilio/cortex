use std::path::{Path, PathBuf};

use domain::{CustomAgent, Folder, PromptRequest, Skill};

use crate::trust::Trusts;

#[derive(Debug, Clone, Default)]
pub struct Extensions {
    pub plugins: Vec<PathBuf>,
    pub trusts: Trusts,
}

impl Extensions {
    pub(crate) fn host(&self, cwd: &Path, home: Option<&Path>) -> domain::Extensions {
        domain::Extensions {
            plugins: self.plugins.clone(),
            home: home.map(Path::to_path_buf),
            trust: self.trusts.of(cwd, home),
            ..Default::default()
        }
    }

    pub(crate) fn brief(&self, mut request: PromptRequest, home: Option<PathBuf>) -> PromptRequest {
        let extensions = self.host(&request.session.cwd, home.as_deref());
        request.host.extensions = (!extensions.is_empty()).then_some(extensions);
        request
    }
}

pub(crate) fn skills(cwd: &Path, extensions: &Extensions, home: Option<&Path>) -> Vec<Skill> {
    let host = extensions.host(cwd, home);
    agent_engine::tools::skill::discover(Folder::at(cwd, &host), home)
        .into_iter()
        .map(|found| Skill { name: found.name, description: found.description })
        .collect()
}

pub(crate) fn agents(cwd: &Path, extensions: &Extensions, home: Option<&Path>) -> Vec<CustomAgent> {
    let host = extensions.host(cwd, home);
    agent_engine::tools::subagent::discover(Folder::at(cwd, &host), home)
        .into_iter()
        .map(|found| CustomAgent { name: found.name, description: found.description })
        .collect()
}

#[cfg(test)]
mod tests {
    use domain::ModelOption;
    use domain::trust::Trust;

    use super::*;

    #[test]
    fn an_installed_plugin_s_commands_are_offered_under_its_name() {
        let dir = tempfile::tempdir().expect("dir");
        let plugin = dir.path().join("acme");
        std::fs::create_dir_all(plugin.join("commands")).expect("commands");
        std::fs::write(plugin.join("commands/ship.md"), "---\ndescription: Ships\n---\nShip it.").expect("command");
        let extensions = Extensions { plugins: vec![plugin], ..Extensions::default() };
        let names: Vec<String> = skills(dir.path(), &extensions, None).into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"acme:ship".into()), "{names:?}");
    }

    #[test]
    fn a_run_gets_the_host_s_plugins() {
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        assert_eq!(Extensions::default().brief(request.clone(), None), request, "nothing to add");
        let extensions = Extensions { plugins: vec!["/plugins/acme".into()], ..Extensions::default() };
        let added = extensions.brief(request, None).host.extensions.expect("extensions");
        assert_eq!(added.plugins, [PathBuf::from("/plugins/acme")]);
    }

    #[test]
    fn a_run_gets_the_home_its_owner_was_given() {
        let request = PromptRequest::plain("/repo".into(), "hi", ModelOption::new("m", "M"));
        let briefed = Extensions::default().brief(request, Some("/users/ana".into()));
        assert_eq!(briefed.host.extensions.and_then(|extensions| extensions.home), Some(PathBuf::from("/users/ana")));
    }

    #[test]
    fn a_run_is_trusted_only_when_the_store_says_so_whatever_the_request_claims() {
        let repo = tempfile::tempdir().expect("repo");
        let mut request = PromptRequest::plain(repo.path().into(), "hi", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { trust: Trust::Trusted, ..Default::default() });
        let extensions = Extensions::default();
        let trust_of = |request: &PromptRequest| {
            let briefed = extensions.brief(request.clone(), None);
            briefed.host.extensions.map(|extensions| extensions.trust).unwrap_or_default()
        };
        assert_eq!(trust_of(&request), Trust::Untrusted, "what the request said is replaced");

        extensions.trusts.set(repo.path(), None, true).expect("trusted");
        assert_eq!(trust_of(&request), Trust::Trusted);
        let mut elsewhere = request;
        elsewhere.session.cwd = tempfile::tempdir().expect("other").path().into();
        assert_eq!(trust_of(&elsewhere), Trust::Untrusted, "another folder has its own answer");
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
