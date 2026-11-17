use std::path::{Path, PathBuf};

use domain::trust::{ContributionKind as Kind, Refusal, Trust};
use domain::{Extensions, ModelOption, PromptRequest};

use super::*;

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(brand::DIR).join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, text).expect("write");
}

fn repository(inside: &Path, name: &str) -> PathBuf {
    let repo = inside.join(name);
    std::fs::create_dir_all(repo.join(".git")).expect("repository");
    repo
}

fn request(cwd: &Path, home: &Path, trust: Trust) -> PromptRequest {
    let mut request = PromptRequest::plain(cwd.to_path_buf(), "", ModelOption::new("m", "M"));
    request.host.extensions = Some(Extensions { home: Some(home.to_path_buf()), trust, ..Extensions::default() });
    request
}

fn everything(repo: &Path) {
    write(repo, "hooks/guard.js", "export const event = \"Stop\"; export default () => ({});");
    write(repo, "settings.json", r#"{"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "make"}]}]}}"#);
    write(repo, "settings.local.json", r#"{"permissions": {"allow": ["Bash(rm:*)"], "deny": ["Read(.env)"]}}"#);
    write(repo, "mcp.json", r#"{"mcpServers": {"jira": {"command": "jira-mcp", "args": ["--stdio"]}}}"#);
    write(repo, "mcp/theirs.js", "export default { tools: {} };");
    write(repo, "providers/acme.js", "throw new Error('a provider file is never run to be listed');");
    write(repo, "tools/deploy.js", "throw new Error('a tool file is never run to be listed');");
    write(repo, "skills/ship/skill.js", "throw new Error('a skill file is never run to be listed');");
    write(repo, "skills/notes/SKILL.md", "---\ndescription: Notes\n---\nTake notes.");
    write(repo, "commands/release.js", "throw new Error('nor a command');");
    write(repo, "agents/reviewer.md", "---\nname: reviewer\npermissionMode: bypassPermissions\n---\nReview.");
    write(repo, "agents/plain.md", "---\nname: plain\n---\nNothing special.");
    write(repo, "rules/style.md", "Use tabs.");
    std::fs::write(repo.join(brand::INSTRUCTIONS), "Use tabs.").expect("instructions");
}

#[test]
fn a_folder_s_root_is_canonical_and_stops_at_its_own_repository() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    let nested = repository(&repo, "vendor/engine");
    std::fs::create_dir_all(repo.join("src")).expect("src");
    let real = crate::tools::settled(&repo);

    assert_eq!(root_of(&repo.join("src"), Some(home.path())), real);
    assert_eq!(root_of(&nested, Some(home.path())), real.join("vendor/engine"));
    assert_eq!(root_of(&repo.join("not/there/yet"), Some(home.path())), real, "a folder to be made counts too");
    #[cfg(unix)]
    {
        let link = work.path().join("shortcut");
        std::os::unix::fs::symlink(&repo, &link).expect("link");
        assert_eq!(root_of(&link.join("src"), Some(home.path())), real, "a link names the folder it points at");
    }
}

#[test]
fn the_home_folder_is_never_a_project_even_when_it_is_a_repository() {
    let home = tempfile::tempdir().expect("home");
    std::fs::create_dir_all(home.path().join(".git")).expect("dotfiles in git");
    write(home.path(), "hooks/mine.js", "export const event = \"Stop\"; export default () => ({});");
    let notes = home.path().join("notes");
    std::fs::create_dir_all(&notes).expect("notes");

    let at_home = report(home.path(), Some(home.path()), Trust::Trusted);
    assert_eq!((at_home.trust, at_home.refusal), (Trust::Untrusted, Some(Refusal::Home)), "trust asked for is refused");
    assert!(at_home.brings.is_empty(), "the user's own folder is not a project's: {:?}", at_home.brings);
    assert!(!at_home.asks());
    assert!(
        Gate::at(home.path(), Some(home.path()), Trust::Untrusted)
            .honours(&home.path().join(brand::DIR).join("hooks/mine.js"))
    );

    assert_eq!(root_of(&notes, Some(home.path())), crate::tools::settled(&notes), "a folder of it is its own root");
}

#[test]
fn the_summary_lists_what_trust_would_enable_and_nothing_that_only_informs_or_restricts() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    everything(&repo);

    let found = report(&repo, Some(home.path()), Trust::Untrusted);

    assert!(found.asks());
    let dir = brand::DIR;
    let listed: Vec<(Kind, String)> = found.brings.items().iter().map(|item| (item.kind, item.what.clone())).collect();
    let expected = [
        (Kind::SettingsHook, format!("Stop *: make ({dir}/settings.json)")),
        (Kind::LocalSettings, "allow Bash(rm:*)".to_owned()),
        (Kind::HookScript, format!("{dir}/hooks/guard.js")),
        (Kind::SkillScript, format!("{dir}/skills/ship/skill.js")),
        (Kind::SkillScript, format!("{dir}/commands/release.js")),
        (Kind::AgentPrivilege, format!("{dir}/agents/reviewer.md: permissionMode")),
        (Kind::McpServer, "jira: jira-mcp --stdio".to_owned()),
        (Kind::McpServer, format!("{dir}/mcp/theirs.js (script)")),
        (Kind::DeclaredProvider, format!("{dir}/providers/acme.js")),
        (Kind::ScriptTool, format!("{dir}/tools/deploy.js")),
    ];
    assert_eq!(listed, expected);
}

#[test]
fn a_local_setting_that_lets_sandboxed_commands_read_credentials_is_a_grant_to_ask_about() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "settings.local.json", r#"{"sandbox": {"allowRead": ["~/.ssh", "~/.aws/credentials"]}}"#);

    let found = report(&repo, Some(home.path()), Trust::Untrusted);

    let said: Vec<&str> = found.brings.items().iter().map(|brought| brought.what.as_str()).collect();
    assert!(
        said.contains(&"sandbox may read ~/.ssh") && said.contains(&"sandbox may read ~/.aws/credentials"),
        "{said:?}"
    );
    assert!(found.asks());
}

#[test]
fn a_folder_with_only_instructions_rules_and_deny_rules_has_nothing_to_ask_about() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "settings.json", r#"{"permissions": {"deny": ["Bash(rm:*)"], "allow": ["Bash(ls:*)"]}}"#);
    write(
        &repo,
        "settings.local.json",
        r#"{"permissions": {"ask": ["Bash(git push:*)"]}, "features": {"experimental-sandbox": true, "trace": false}}"#,
    );
    write(&repo, "rules/style.md", "Use tabs.");
    write(&repo, "skills/notes/SKILL.md", "---\ndescription: Notes\n---\nTake notes.");
    write(&repo, "agents/plain.md", "---\nname: plain\n---\nNothing special.");
    std::fs::write(repo.join(brand::INSTRUCTIONS), "Use tabs.").expect("instructions");

    let found = report(&repo, Some(home.path()), Trust::Untrusted);

    assert!(found.brings.is_empty(), "{:?}", found.brings);
    assert!(!found.asks());
    assert_eq!(Gate::at(&repo, Some(home.path()), Trust::Untrusted).skipped_notice(), None);
}

#[test]
fn an_untrusted_folder_s_scripts_agents_and_servers_are_left_out_and_its_text_is_still_read() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    everything(&repo);
    write(&repo, "skills/ship/skill.js", "export const description = \"Ships\"; export default () => \"Ship it.\";");
    write(&repo, "commands/release.js", "export const description = \"Releases\"; export default () => \"Go.\";");
    write(&repo, "settings.local.json", r#"{"enableAllProjectMcpServers": true}"#);

    let untrusted = request(&repo, home.path(), Trust::Untrusted);
    let brief = crate::brief::Brief::load(untrusted.folder()).expect("brief");
    let skills: Vec<&str> = brief.skills.iter().map(|skill| skill.name.as_str()).collect();
    assert!(skills.contains(&"notes"), "a Markdown skill still loads: {skills:?}");
    assert!(!skills.contains(&"ship") && !skills.contains(&"release"), "a script skill does not: {skills:?}");
    let reviewer = brief.agents.iter().find(|agent| agent.name == "reviewer").expect("the agent is still offered");
    assert_eq!(reviewer.config.permission, None, "without the mode its file asked for");
    assert_eq!(reviewer.instructions, "Review.");
    assert!(brief.instructions.iter().any(|file| file.text.contains("Use tabs.")), "the instruction file is read");
    assert!(brief.rules.iter().any(|rule| rule.text.contains("Use tabs.")), "and so are its rules");
    let skipped = crate::mcp::list(untrusted.folder()).expect("list");
    assert!(
        skipped.iter().all(|server| server.state == domain::mcp_config::McpState::Untrusted),
        "its servers are only shown as skipped: {skipped:?}"
    );
    assert!(crate::mcp::servers(untrusted.folder(), &brief.mcp()).is_empty(), "nor started");
    assert!(crate::tools::script_tool::load(&brief.gate(&repo)).is_empty());

    let trusted = request(&repo, home.path(), Trust::Trusted);
    let brief = crate::brief::Brief::load(trusted.folder()).expect("brief");
    let skills: Vec<&str> = brief.skills.iter().map(|skill| skill.name.as_str()).collect();
    assert!(skills.contains(&"ship") && skills.contains(&"release"), "{skills:?}");
    let reviewer = brief.agents.iter().find(|agent| agent.name == "reviewer").expect("offered");
    assert_eq!(reviewer.config.permission, Some(domain::PermissionMode::Bypass));
    let listed: Vec<String> =
        crate::mcp::list(trusted.folder()).expect("list").into_iter().map(|server| server.name).collect();
    assert_eq!(listed, ["jira", "theirs"]);
    let started: Vec<String> =
        crate::mcp::servers(trusted.folder(), &brief.mcp()).iter().map(|server| server.name().to_owned()).collect();
    assert_eq!(started, ["jira", "theirs"], "approved in its local settings, which trust made the owner's");
}

#[test]
fn a_skill_with_a_script_and_a_markdown_file_is_offered_as_markdown_while_untrusted() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "skills/ship/skill.js", "throw new Error('never run while untrusted');");
    write(&repo, "skills/ship/SKILL.md", "---\ndescription: Ships by hand\n---\nShip it.");

    let brief = crate::brief::Brief::load(request(&repo, home.path(), Trust::Untrusted).folder()).expect("brief");

    let ship = brief.skills.iter().find(|skill| skill.name == "ship").expect("the Markdown skill is offered");
    assert_eq!(ship.description, "Ships by hand");
    assert!(ship.file.extension().is_some_and(|extension| extension == "md"), "{:?}", ship.file);
}

#[test]
fn a_trusted_folder_s_server_still_waits_for_its_own_approval() {
    let (home, work) = (tempfile::tempdir().expect("home"), tempfile::tempdir().expect("work"));
    let repo = repository(work.path(), "rocket");
    write(&repo, "mcp.json", r#"{"mcpServers": {"jira": {"command": "jira-mcp"}}}"#);

    let trusted = request(&repo, home.path(), Trust::Trusted);
    let listed = crate::mcp::list(trusted.folder()).expect("list");

    assert_eq!(listed[0].state, domain::mcp_config::McpState::Pending, "trust is not the server's approval");
    let brief = crate::brief::Brief::load(trusted.folder()).expect("brief");
    assert!(crate::mcp::servers(trusted.folder(), &brief.mcp()).is_empty());
}
