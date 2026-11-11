use std::path::PathBuf;

use crate::{prompt, tools};
use domain::PromptRequest;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brief {
    pub skills: Vec<tools::skill::FoundSkill>,
    pub agents: Vec<tools::subagent::FoundAgent>,
    pub settings: prompt::settings::Settings,
    pub rules: Vec<prompt::rules::Rule>,
    pub git: Option<prompt::GitSnapshot>,
    pub instructions: Vec<prompt::Instructions>,
    #[serde(default)]
    pub plugins: Vec<crate::plugins::Plugin>,
    #[serde(default)]
    pub home: Option<PathBuf>,
}
impl Brief {
    pub fn load(request: &PromptRequest) -> Result<Self, String> {
        let home = home(request);
        let home = home.as_deref();
        let plugins = crate::plugins::found(request, home);
        Ok(Self {
            skills: tools::skill::discover(request, home),
            agents: tools::subagent::discover(request, home),
            settings: prompt::settings::load(&request.cwd, home, &plugins)?,
            rules: prompt::rules::discover(&request.cwd, home),
            git: prompt::snapshot(&request.cwd),
            instructions: prompt::instructions(&request.cwd, home),
            plugins,
            home: home.map(PathBuf::from),
        })
    }

    pub fn mcp(&self) -> crate::mcp::McpSources {
        crate::mcp::McpSources {
            home: self.home.clone(),
            plugins: self.plugins.clone(),
            approval: self.settings.mcp_approval.clone(),
            scripts: self.settings.scripts,
        }
    }
}

pub fn mcp_sources(request: &PromptRequest) -> Result<crate::mcp::McpSources, String> {
    let home = home(request);
    let plugins = crate::plugins::found(request, home.as_deref());
    let settings = prompt::settings::load(&request.cwd, home.as_deref(), &plugins)?;
    Ok(crate::mcp::McpSources { home, plugins, approval: settings.mcp_approval, scripts: settings.scripts })
}

pub fn permissions(request: &PromptRequest) -> Result<domain::permission_rules::PermissionsInfo, String> {
    let home = home(request);
    let plugins = crate::plugins::found(request, home.as_deref());
    let settings = prompt::settings::load(&request.cwd, home.as_deref(), &plugins)?;
    Ok(domain::permission_rules::PermissionsInfo {
        rules: settings.permissions.listed(),
        added_dirs: settings.added_dirs,
        hooks: settings.hooks.listed(),
    })
}

pub fn home(request: &PromptRequest) -> Option<PathBuf> {
    request.extensions.as_ref().and_then(|extensions| extensions.home.clone()).or_else(std::env::home_dir)
}

pub fn added_dirs(request: &PromptRequest, settings: &[String], home: Option<&std::path::Path>) -> Vec<PathBuf> {
    let from_settings = settings.iter().filter_map(|dir| match dir.strip_prefix("~/") {
        Some(rest) => home.map(|home| home.join(rest)),
        None => Some(request.cwd.join(dir)),
    });
    request.add_dirs.iter().map(|dir| request.cwd.join(dir)).chain(from_settings).collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use domain::{ModelOption, PromptRequest};

    #[test]
    fn added_folders_come_from_the_session_and_settings() {
        let mut request = PromptRequest::plain("/work/rocket".into(), "hi", ModelOption::new("m", "M"));
        request.add_dirs = vec!["/shared".into()];
        let settings = ["~/notes".to_owned(), "../lib".to_owned(), "/opt/x".to_owned()];
        let dirs = super::added_dirs(&request, &settings, Some(Path::new("/home/ana")));
        let expected: Vec<PathBuf> =
            ["/shared", "/home/ana/notes", "/work/rocket/../lib", "/opt/x"].iter().map(PathBuf::from).collect();
        assert_eq!(dirs, expected);
    }

    #[test]
    fn the_permissions_of_a_folder_are_its_settings_rules() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(brand::DIR)).expect("dir");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/settings.local.json")),
            r#"{"permissions": {"allow": ["Bash(ls:*)"], "additionalDirectories": ["../lib"]},
                "hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "./check.sh"}]}]}}"#,
        )
        .expect("write");
        let mut request = PromptRequest::plain(repo.path().into(), "", ModelOption::new("m", "M"));
        request.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });
        let info = super::permissions(&request).expect("permissions");
        assert_eq!(info.rules, [(domain::permission_rules::RuleKind::Allow, "Bash(ls:*)".to_owned())]);
        assert_eq!(info.added_dirs, ["../lib"]);
        let hook = domain::permission_rules::HookInfo {
            event: "PreToolUse".into(),
            matcher: "Bash".into(),
            runs: "./check.sh".into(),
        };
        assert!(info.hooks.contains(&hook), "{:?}", info.hooks);
    }
}
