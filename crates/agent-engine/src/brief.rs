use std::path::PathBuf;

use crate::trust::Gate;
use crate::{prompt, tools};
use domain::Folder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BriefError {
    #[error("{0}")]
    Settings(String),
    #[error("a Brief hook stopped the session: {0}")]
    StoppedByHook(String),
    #[error("a Brief hook: {0}")]
    EditedBadly(String),
    #[error("{0}")]
    Hook(String),
}

impl From<BriefError> for String {
    fn from(error: BriefError) -> Self {
        error.to_string()
    }
}

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
    #[serde(default)]
    pub trust: domain::trust::Trust,
}
impl Brief {
    pub fn load(folder: Folder<'_>) -> Result<Self, BriefError> {
        let gate = Gate::of(folder);
        let home = gate.home();
        let plugins = crate::plugins::found(folder, home);
        let settings = prompt::settings::load(&gate, &plugins).map_err(BriefError::Settings)?;
        let mut skills = tools::skill::discover(folder, home);
        skills.retain(|skill| domain::features::skill_flag(&skill.name).is_none_or(|flag| settings.features.on(flag)));
        let found = crate::hooks::Briefed {
            skills,
            agents: tools::subagent::discover(folder, home),
            rules: prompt::rules::discover(folder.cwd, home),
            instructions: prompt::instructions(folder.cwd, home),
        };
        let edited = settings.hooks.edit_brief(found, &hook_workspace(folder.cwd, home, &settings))?;
        Ok(Self {
            skills: edited.skills,
            agents: edited.agents,
            settings,
            rules: edited.rules,
            git: prompt::snapshot(folder.cwd),
            instructions: edited.instructions,
            plugins,
            home: home.map(PathBuf::from),
            trust: gate.trust(),
        })
    }

    pub fn mcp(&self) -> crate::mcp::McpSources {
        crate::mcp::McpSources {
            home: self.home.clone(),
            plugins: self.plugins.clone(),
            approval: self.settings.mcp_approval.clone(),
            scripts: self.settings.scripts,
            trust: self.trust,
            sandbox: sandbox_of(&self.settings),
        }
    }

    pub fn gate(&self, cwd: &std::path::Path) -> Gate {
        Gate::at(cwd, self.home.as_deref(), self.trust)
    }
}

fn hook_workspace(
    cwd: &std::path::Path,
    home: Option<&std::path::Path>,
    settings: &prompt::settings::Settings,
) -> tools::Workspace {
    let mut workspace = tools::Workspace::new(cwd);
    workspace.home = home.map(PathBuf::from);
    workspace.policy.scripts = settings.scripts;
    workspace
}

pub fn mcp_sources(folder: Folder<'_>) -> Result<crate::mcp::McpSources, BriefError> {
    let gate = Gate::of(folder);
    let plugins = crate::plugins::found(folder, gate.home());
    let settings = prompt::settings::load(&gate, &plugins).map_err(BriefError::Settings)?;
    let sandbox = sandbox_of(&settings);
    Ok(crate::mcp::McpSources {
        home: gate.home().map(PathBuf::from),
        plugins,
        approval: settings.mcp_approval,
        scripts: settings.scripts,
        trust: gate.trust(),
        sandbox,
    })
}

fn sandbox_of(settings: &prompt::settings::Settings) -> Option<domain::sandbox::Sandbox> {
    settings.features.on(domain::features::SANDBOX).then(|| settings.sandbox.clone())
}

pub fn permissions(folder: Folder<'_>) -> Result<domain::permission_rules::PermissionsInfo, BriefError> {
    let gate = Gate::of(folder);
    let plugins = crate::plugins::found(folder, gate.home());
    let settings = prompt::settings::load(&gate, &plugins).map_err(BriefError::Settings)?;
    Ok(domain::permission_rules::PermissionsInfo {
        rules: settings.permissions.listed(),
        added_dirs: settings.added_dirs,
        hooks: settings.hooks.listed(),
        notices: settings.notices,
        skipped_files: settings.skipped_files,
    })
}

pub fn home(folder: Folder<'_>) -> Option<PathBuf> {
    folder.home().map(PathBuf::from)
}

pub fn added_dirs(
    session: &domain::SessionConfig,
    settings: &[String],
    home: Option<&std::path::Path>,
) -> Vec<PathBuf> {
    let from_settings = settings.iter().filter_map(|dir| match dir.strip_prefix("~/") {
        Some(rest) => home.map(|home| home.join(rest)),
        None => Some(session.cwd.join(dir)),
    });
    session.add_dirs.iter().map(|dir| session.cwd.join(dir)).chain(from_settings).collect()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use domain::{ModelOption, PromptRequest};

    #[test]
    fn added_folders_come_from_the_session_and_settings() {
        let mut request = PromptRequest::plain("/work/rocket".into(), "hi", ModelOption::new("m", "M"));
        request.session.add_dirs = vec!["/shared".into()];
        let settings = ["~/notes".to_owned(), "../lib".to_owned(), "/opt/x".to_owned()];
        let dirs = super::added_dirs(&request.session, &settings, Some(Path::new("/home/ana")));
        let expected: Vec<PathBuf> =
            ["/shared", "/home/ana/notes", "/work/rocket/../lib", "/opt/x"].iter().map(PathBuf::from).collect();
        assert_eq!(dirs, expected);
    }

    #[test]
    fn a_run_reads_only_the_home_its_backend_gave_it() {
        let mut request = PromptRequest::plain("/work/rocket".into(), "hi", ModelOption::new("m", "M"));
        assert_eq!(super::home(request.folder()), None, "never the machine's own home");
        request.host.extensions = Some(domain::Extensions { home: Some("/users/ana".into()), ..Default::default() });
        assert_eq!(super::home(request.folder()), Some(PathBuf::from("/users/ana")));
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
        request.host.extensions = Some(domain::Extensions {
            home: Some(home.path().into()),
            trust: domain::trust::Trust::Trusted,
            ..Default::default()
        });
        let info = super::permissions(request.folder()).expect("permissions");
        assert_eq!(info.rules, [(domain::permission_rules::RuleKind::Allow, "Bash(ls:*)".to_owned())]);
        assert_eq!(info.added_dirs, ["../lib"]);
        let hook = domain::permission_rules::HookInfo {
            event: crate::hooks::HookEvent::PreToolUse,
            matcher: "Bash".into(),
            runs: "./check.sh".into(),
        };
        assert!(info.hooks.contains(&hook), "{:?}", info.hooks);
    }

    #[test]
    fn a_skill_behind_a_flag_is_offered_until_the_owner_turns_the_flag_off() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let mut request = PromptRequest::plain(repo.path().into(), "", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });
        let offered = |request: &PromptRequest| {
            super::Brief::load(request.folder()).expect("brief").skills.iter().any(|skill| skill.name == "follow-pr")
        };
        assert!(offered(&request));

        std::fs::create_dir_all(home.path().join(brand::DIR)).expect("dir");
        let off = serde_json::json!({ "features": { domain::features::PR_FOLLOW: false } });
        std::fs::write(home.path().join(brand::DIR).join("settings.json"), off.to_string()).expect("settings");
        assert!(!offered(&request));
    }

    #[test]
    fn a_brief_hook_adds_changes_and_removes_the_session_s_skills() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let hooks = home.path().join(brand::DIR).join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks");
        std::fs::write(
            hooks.join("skills.js"),
            r#"export const event = "Brief";
export default (input) => {
  const review = input.skills.find((skill) => skill.name === "review");
  return {
    skills: [
      ...input.skills
        .filter((skill) => skill.name !== "follow-pr" && skill.name !== "review")
        .map((skill) => ({ name: skill.name })),
      { name: "review", description: `ACME: ${review.description}`, body: review.path.length > 0 ? "Use the ACME checklist." : "" },
      { name: "deploy", description: "Ship it", body: "Run make deploy." },
    ],
  };
};"#,
        )
        .expect("hook");
        let mut request = PromptRequest::plain(repo.path().into(), "", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });

        let skills = super::Brief::load(request.folder()).expect("brief").skills;

        let named = |name: &str| skills.iter().find(|skill| skill.name == name);
        assert!(named("follow-pr").is_none(), "removed");
        assert!(named("verify").is_some(), "kept");
        let review = named("review").expect("review");
        assert!(review.description.starts_with("ACME: "), "{}", review.description);
        assert_eq!(review.body.as_deref(), Some("Use the ACME checklist."));
        let deploy = named("deploy").expect("deploy");
        assert_eq!((deploy.description.as_str(), deploy.body.as_deref()), ("Ship it", Some("Run make deploy.")));
    }

    #[test]
    fn a_brief_hook_replaces_the_agents_rules_and_instructions_and_hooks_run_in_turn() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::write(repo.path().join(brand::INSTRUCTIONS), "Use tabs.").expect("instructions");
        let hooks = home.path().join(brand::DIR).join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks");
        std::fs::write(
            hooks.join("a-team.js"),
            r#"export const event = "Brief";
export default (input) => ({
  agents: [
    ...input.agents.filter((agent) => agent.name !== "explore"),
    { name: "reviewer", description: "Reviews diffs", instructions: "Be strict." },
  ],
  rules: [...input.rules, { path: "/team/rust.md", text: "No unwrap.", patterns: ["**/*.rs"] }],
  instructions: input.instructions.map((found) => ({ ...found, text: `${found.text} Team ACME.` })),
});"#,
        )
        .expect("hook");
        std::fs::write(
            hooks.join("b-after.js"),
            r#"export const event = "Brief";
export default (input) => ({
  agents: input.agents.map((agent) => agent.name === "reviewer" ? { ...agent, description: "Seen by the next hook" } : agent),
});"#,
        )
        .expect("hook");
        let mut request = PromptRequest::plain(repo.path().into(), "", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });

        let brief = super::Brief::load(request.folder()).expect("brief");

        let agents: Vec<(&str, &str)> =
            brief.agents.iter().map(|agent| (agent.name.as_str(), agent.description.as_str())).collect();
        assert!(!agents.iter().any(|(name, _)| *name == "explore"), "{agents:?}");
        assert!(agents.contains(&("reviewer", "Seen by the next hook")), "{agents:?}");
        let rule = brief.rules.last().expect("rule");
        assert_eq!((rule.text.as_str(), rule.patterns.as_slice()), ("No unwrap.", ["**/*.rs".to_owned()].as_slice()));
        let texts: Vec<&str> = brief.instructions.iter().map(|found| found.text.as_str()).collect();
        assert!(texts.contains(&"Use tabs. Team ACME."), "{texts:?}");
    }

    #[test]
    fn a_brief_hook_that_names_an_agent_twice_stops_the_session() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let hooks = home.path().join(brand::DIR).join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks");
        std::fs::write(
            hooks.join("twice.js"),
            r#"export const event = "Brief";
export default () => ({ agents: [
  { name: "a", description: "x", instructions: "y" },
  { name: "a", description: "x", instructions: "y" },
] });"#,
        )
        .expect("hook");
        let mut request = PromptRequest::plain(repo.path().into(), "", ModelOption::new("m", "M"));
        request.host.extensions = Some(domain::Extensions { home: Some(home.path().into()), ..Default::default() });

        let error = super::Brief::load(request.folder()).map(|_| ()).expect_err("twice");

        assert_eq!(error, super::BriefError::EditedBadly("the hook listed the agent \"a\" twice".into()));
        assert_eq!(String::from(error), "a Brief hook: the hook listed the agent \"a\" twice");
    }

    #[test]
    fn each_brief_failure_reads_as_the_text_it_always_had() {
        use super::BriefError;
        assert_eq!(BriefError::Settings("settings.json: bad".into()).to_string(), "settings.json: bad");
        assert_eq!(
            BriefError::StoppedByHook("not today".into()).to_string(),
            "a Brief hook stopped the session: not today"
        );
        assert_eq!(BriefError::Hook("hook worker stopped".into()).to_string(), "hook worker stopped");
    }
}
