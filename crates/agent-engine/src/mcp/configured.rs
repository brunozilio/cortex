use std::path::{Path, PathBuf};

use domain::Folder;
use domain::mcp_config::{
    McpApproval, McpListing, McpOrigin, McpServerInfo, McpState, McpUnreadable, expand, is_free_name,
};
use serde_json::Value;

use super::{ServerSpec, parse_config};
use crate::plugins::Plugin;
use crate::tools::script_tool::Origin;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct McpSources {
    pub home: Option<PathBuf>,
    pub plugins: Vec<Plugin>,
    pub approval: McpApproval,
    #[serde(default)]
    pub scripts: crate::tools::script_tool::ScriptGrants,
    #[serde(default)]
    pub trust: domain::trust::Trust,
    #[serde(default)]
    pub sandbox: Option<domain::sandbox::Sandbox>,
}

fn places<'a>(
    sources: &'a McpSources,
    cwd: &'a Path,
    temp: &'a [PathBuf],
    data: &'a [PathBuf],
) -> domain::sandbox::Places<'a> {
    domain::sandbox::Places {
        project: cwd,
        added: &[],
        repository: None,
        temp,
        home: sources.home.as_deref(),
        settings_dir: brand::DIR,
        data,
    }
}

fn confinement(sources: &McpSources, cwd: &Path) -> Option<platform::sandbox::Confinement> {
    let sandbox = sources.sandbox.as_ref()?;
    let (temp, data) = (crate::tools::temp_folders(), crate::tools::data_folders());
    Some(crate::tools::confinement_of(sandbox, &places(sources, cwd, &temp, &data)))
}

pub(super) fn settled(server: ServerSpec, owner: bool, cwd: &Path, sources: &McpSources) -> ServerSpec {
    settled_on(server, owner, cwd, sources, &platform::sandbox::support())
}

pub(super) fn settled_on(
    server: ServerSpec,
    owner: bool,
    cwd: &Path,
    sources: &McpSources,
    support: &platform::sandbox::Support,
) -> ServerSpec {
    let ServerSpec::Stdio { name, command, args, env, sandbox } = server else { return server };
    let (temp, data) = (crate::tools::temp_folders(), crate::tools::data_folders());
    let places = places(sources, cwd, &temp, &data);
    let settling = super::stdio_sandbox::Settled { sandbox: sources.sandbox.as_ref(), places: &places, support };
    ServerSpec::Stdio { name, command, args, env, sandbox: sandbox.settled(owner, &settling) }
}

pub fn configured(cwd: &Path, sources: &McpSources) -> Vec<ServerSpec> {
    configured_on(cwd, sources, &platform::sandbox::support())
}

fn configured_on(cwd: &Path, sources: &McpSources, support: &platform::sandbox::Support) -> Vec<ServerSpec> {
    origins(cwd, sources)
        .into_iter()
        .filter(|(origin, server)| McpState::of(origin, server.name(), &sources.approval) == McpState::Ready)
        .map(|(origin, server)| settled_on(server, origin == McpOrigin::User, cwd, sources, support))
        .collect()
}

pub fn sources(folder: Folder<'_>) -> Result<McpSources, crate::brief::BriefError> {
    crate::brief::mcp_sources(folder)
}

pub fn list(folder: Folder<'_>) -> Result<Vec<McpServerInfo>, crate::brief::BriefError> {
    listing(folder).map(|listing| listing.servers)
}

pub fn listing(folder: Folder<'_>) -> Result<McpListing, crate::brief::BriefError> {
    let sources = crate::brief::mcp_sources(folder)?;
    let cwd = folder.cwd;
    let home = sources.home.as_deref();
    let mut listing = McpListing::default();
    if let Some(path) = project_config(cwd, home) {
        match parsed(&path) {
            Err(error) => listing.unreadable.push(McpUnreadable::broken(path, McpOrigin::Project, error)),
            Ok(found) => {
                let left_out = found.problems.iter();
                listing
                    .unreadable
                    .extend(left_out.map(|left| McpUnreadable::left_out(path.clone(), McpOrigin::Project, left)));
                if !crate::trust::Gate::at(cwd, home, sources.trust).honours(&path) {
                    let free = found.servers.into_iter().filter(|server| is_free_name(server.name()));
                    listing.servers.extend(free.map(|server| McpServerInfo {
                        name: server.name().to_owned(),
                        origin: McpOrigin::Project,
                        target: server.target(),
                        state: McpState::Untrusted,
                    }));
                }
            }
        }
    }
    let user = home.map(|home| home.join(brand::DIR).join("mcp.json")).filter(|path| path.is_file());
    if let Some(path) = user {
        match parsed(&path) {
            Err(error) => listing.unreadable.push(McpUnreadable::broken(path, McpOrigin::User, error)),
            Ok(found) => listing
                .unreadable
                .extend(found.problems.iter().map(|left| McpUnreadable::left_out(path.clone(), McpOrigin::User, left))),
        }
    }
    listing.servers.extend(origins(cwd, &sources).into_iter().map(|(origin, server)| McpServerInfo {
        name: server.name().to_owned(),
        state: McpState::of(&origin, server.name(), &sources.approval),
        target: server.target(),
        origin,
    }));
    Ok(listing)
}

pub fn project_config(cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    cwd.ancestors()
        .filter(|dir| Some(*dir) != home)
        .map(|dir| dir.join(brand::DIR).join("mcp.json"))
        .find(|path| path.is_file())
}

fn origins(cwd: &Path, sources: &McpSources) -> Vec<(McpOrigin, ServerSpec)> {
    let gate = crate::trust::Gate::at(cwd, sources.home.as_deref(), sources.trust);
    let project = project_config(cwd, sources.home.as_deref()).filter(|path| gate.honours(path));
    let project = project.map(|path| read(&path, "the project's")).unwrap_or_default();
    let user = sources
        .home
        .as_ref()
        .map(|home| read(&home.join(concat!(brand::dir!(), "/mcp.json")), "the user's"))
        .unwrap_or_default();
    let mut found: Vec<(McpOrigin, ServerSpec)> = Vec::new();
    let scripts_in = |root: &Path, origin: Origin| {
        let folder = root.join(brand::DIR).join("mcp");
        if !gate.honours(&folder) {
            return Vec::new();
        }
        let granted = super::script_server::Granted {
            access: sources.scripts.of(origin),
            origin,
            confinement: confinement(sources, cwd),
        };
        super::script_server::found(&folder, cwd, &granted)
    };
    let project_scripts = scripts_in(cwd, Origin::Project);
    let user_scripts = sources.home.as_ref().map(|home| scripts_in(home, Origin::Owner)).unwrap_or_default();
    let project = project.into_iter().chain(project_scripts);
    let user = user.into_iter().chain(user_scripts);
    let own = project.map(|s| (McpOrigin::Project, s)).chain(user.map(|s| (McpOrigin::User, s)));
    for (origin, server) in own {
        let taken = found.iter().any(|(known_origin, known)| {
            known.name() == server.name()
                && McpState::of(known_origin, known.name(), &sources.approval) == McpState::Ready
        });
        if is_free_name(server.name()) && !taken {
            found.push((origin, server));
        }
    }
    for plugin in &sources.plugins {
        let origin = McpOrigin::Plugin(plugin.name.clone());
        found.extend(plugin_servers(plugin).into_iter().map(|server| (origin.clone(), server)));
    }
    found
}

fn plugin_servers(plugin: &Plugin) -> Vec<ServerSpec> {
    let Some(config) = plugin.mcp_config() else { return Vec::new() };
    match parse_config(&config.to_string()) {
        Ok(found) => found
            .into_iter()
            .filter(|s| !s.name().starts_with(concat!(brand::command!(), "_")))
            .map(|s| s.renamed(plugin))
            .collect(),
        Err(error) => {
            tracing::warn!(%error, plugin = %plugin.name, "a plugin's MCP servers are not available");
            Vec::new()
        }
    }
}

struct Parsed {
    servers: Vec<ServerSpec>,
    problems: Vec<String>,
}

fn parsed(path: &Path) -> Result<Parsed, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let config: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let checked = crate::config_schema::checked_mcp(config);
    let config = expand(checked.value, &|name| std::env::var(name).ok());
    Ok(Parsed { servers: parse_config(&config.to_string())?, problems: checked.problems })
}

fn read(path: &Path, whose: &str) -> Vec<ServerSpec> {
    match parsed(path) {
        Ok(found) => {
            let located: Vec<String> =
                found.problems.iter().map(|problem| format!("{}: {problem}", path.display())).collect();
            crate::config_schema::logged_once(&located);
            found.servers
        }
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "{whose} MCP servers are not available");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::{ModelOption, PromptRequest};

    use super::*;
    use crate::mcp::servers;

    fn write(path: PathBuf, text: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, text).expect("write");
    }

    fn stdio(name: &str, command: &str, env: &[(&str, &str)]) -> ServerSpec {
        ServerSpec::Stdio {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            sandbox: crate::mcp::StdioSandbox::Unconfined,
        }
    }

    #[test]
    fn the_home_folder_s_config_is_the_user_s_never_a_project_s() {
        let home = tempfile::tempdir().expect("home");
        write(home.path().join(concat!(brand::dir!(), "/mcp.json")), r#"{"mcpServers": {}}"#);
        let repo = home.path().join("code/app");
        std::fs::create_dir_all(&repo).expect("repo");
        assert_eq!(project_config(&repo, Some(home.path())), None);
        write(home.path().join(concat!("code/", brand::dir!(), "/mcp.json")), r#"{"mcpServers": {}}"#);
        assert_eq!(
            project_config(&repo, Some(home.path())),
            Some(home.path().join(concat!("code/", brand::dir!(), "/mcp.json")))
        );
    }

    #[test]
    fn the_listing_shows_every_server_its_origin_and_whether_it_starts() {
        use domain::Extensions;
        use domain::mcp_config::{McpOrigin, McpServerInfo, McpState};
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let plugin = tempfile::tempdir().expect("plugin");
        write(
            home.path().join(concat!(brand::dir!(), "/mcp.json")),
            r#"{"mcpServers": {"atlassian": {"type": "http", "url": "https://mcp.atlassian.com/v1/mcp", "headers": {"Authorization": "Bearer secret"}}}}"#,
        );
        write(
            repo.path().join(concat!(brand::dir!(), "/mcp.json")),
            r#"{"mcpServers": {"jira": {"command": "jira-mcp", "args": ["--stdio"]}, "sonar": {"command": "sonar-mcp"}}}"#,
        );
        write(repo.path().join(concat!(brand::dir!(), "/settings.local.json")), r#"{"disabledMcpServers": ["sonar"]}"#);
        write(plugin.path().join(concat!(brand::plugin_dir!(), "/plugin.json")), r#"{"name": "acme"}"#);
        write(plugin.path().join(".mcp.json"), r#"{"mcpServers": {"datadog": {"command": "dd-mcp"}}}"#);
        let mut request = PromptRequest::plain(repo.path().to_path_buf(), "", ModelOption::new("", ""));
        request.host.extensions = Some(Extensions {
            home: Some(home.path().to_path_buf()),
            plugins: vec![plugin.path().to_path_buf()],
            trust: domain::trust::Trust::Trusted,
            ..Extensions::default()
        });
        let info = |name: &str, origin: McpOrigin, target: &str, state: McpState| McpServerInfo {
            name: name.into(),
            origin,
            target: target.into(),
            state,
        };
        let listed = crate::mcp::list(request.folder()).expect("listed");
        assert_eq!(
            listed,
            [
                info("jira", McpOrigin::Project, "jira-mcp --stdio", McpState::Pending),
                info("sonar", McpOrigin::Project, "sonar-mcp", McpState::Off),
                info("atlassian", McpOrigin::User, "https://mcp.atlassian.com/v1/mcp", McpState::Ready),
                info("plugin_acme_datadog", McpOrigin::Plugin("acme".into()), "dd-mcp", McpState::Ready),
            ]
        );
        assert!(!format!("{listed:?}").contains("secret"), "headers stay out");
    }

    #[test]
    fn the_listing_names_each_unreadable_file_and_an_untrusted_project_s_servers_as_skipped() {
        use domain::Extensions;
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let user = home.path().join(concat!(brand::dir!(), "/mcp.json"));
        write(user.clone(), "{broken");
        write(repo.path().join(concat!(brand::dir!(), "/mcp.json")), r#"{"mcpServers": {"jira": {"command": "j"}}}"#);
        let mut request = PromptRequest::plain(repo.path().to_path_buf(), "", ModelOption::new("", ""));
        request.host.extensions = Some(Extensions {
            home: Some(home.path().to_path_buf()),
            trust: domain::trust::Trust::Untrusted,
            ..Extensions::default()
        });
        let listing = crate::mcp::listing(request.folder()).expect("listed");
        assert_eq!(
            listing.servers,
            [McpServerInfo {
                name: "jira".into(),
                origin: McpOrigin::Project,
                target: "j".into(),
                state: McpState::Untrusted
            }]
        );
        assert_eq!(listing.unreadable.len(), 1);
        assert_eq!((&listing.unreadable[0].path, &listing.unreadable[0].origin), (&user, &McpOrigin::User));
        assert!(listing.unreadable[0].error.contains("key must be a string"), "{listing:?}");
    }

    #[test]
    fn a_wrong_value_in_an_mcp_file_is_a_warning_and_the_other_servers_still_start() {
        use domain::Extensions;
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let user = home.path().join(concat!(brand::dir!(), "/mcp.json"));
        write(
            user.clone(),
            r#"{"$schema": "./schemas/mcp.schema.json", "mcpServers": {
                "notes": {"command": "notes-mcp", "args": "--stdio"},
                "docs": {"type": "websocket", "url": "wss://docs"},
                "wiki": {"command": "wiki-mcp", "timeout": 5}
            }}"#,
        );
        let project = repo.path().join(concat!(brand::dir!(), "/mcp.json"));
        write(project.clone(), r#"{"servers": {}, "mcpServers": {"jira": {"command": "j", "env": {"N": "1"}}}}"#);
        let mut request = PromptRequest::plain(repo.path().to_path_buf(), "", ModelOption::new("", ""));
        request.host.extensions = Some(Extensions {
            home: Some(home.path().to_path_buf()),
            trust: domain::trust::Trust::Trusted,
            ..Extensions::default()
        });

        let listing = crate::mcp::listing(request.folder()).expect("listed");

        assert!(!listing.owner_s_file_is_broken(), "{listing:?}");
        let warned: Vec<String> = listing.warnings().map(McpUnreadable::line).collect();
        let at = |path: &PathBuf, problem: &str| format!("{}: {problem}", path.display());
        assert_eq!(
            warned,
            [
                at(&project, "servers: unknown key, ignored"),
                at(&user, "mcpServers.docs.type: expected one of \"stdio\", \"http\", \"sse\", found \"websocket\""),
                at(&user, "mcpServers.notes.args: expected array, found \"--stdio\""),
                at(&user, "mcpServers.wiki.timeout: unknown key, ignored"),
            ]
        );
        let names: Vec<&str> = listing.servers.iter().map(|server| server.name.as_str()).collect();
        assert_eq!(names, ["jira", "wiki"], "a server with a wrong value is left out, the others kept");
    }

    #[test]
    fn a_run_gets_the_project_s_approved_servers_and_the_user_s() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        write(
            home.path().join(concat!(brand::dir!(), "/mcp.json")),
            concat!(
                r#"{"mcpServers": {
                "notes": {"command": "notes-mcp", "env": {"KEY": "${"#,
                brand::env_prefix!(),
                r#"_TEST_NEVER_SET:-dflt}"}},
                "jira": {"command": "user-jira"},
                "plugin_fake": {"command": "x"}
            }}"#
            ),
        );
        write(
            repo.path().join(concat!(brand::dir!(), "/mcp.json")),
            concat!(
                r#"{"mcpServers": {
                "jira": {"command": "jira-mcp"},
                "sonar": {"command": "sonar-mcp"},
                ""#,
                brand::command!(),
                r#"_memory": {"command": "evil"}
            }}"#
            ),
        );
        let cwd = repo.path().join("crates/app");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(repo.path().join(".git")).expect("a repository");
        let request = PromptRequest::plain(cwd, "hi", ModelOption::new("m", "M"));
        let mut sources = McpSources {
            home: Some(home.path().to_path_buf()),
            trust: domain::trust::Trust::Trusted,
            ..McpSources::default()
        };

        assert_eq!(
            servers(request.folder(), &sources),
            [stdio("jira", "user-jira", &[]), stdio("notes", "notes-mcp", &[("KEY", "dflt")])],
            "an unapproved project brings nothing; the user's own run"
        );
        sources.approval.enabled = vec!["jira".into(), concat!(brand::command!(), "_memory").into()];
        assert_eq!(
            servers(request.folder(), &sources),
            [stdio("jira", "jira-mcp", &[]), stdio("notes", "notes-mcp", &[("KEY", "dflt")])],
            "the approved project server wins its name; reserved names never start"
        );
        sources.trust = domain::trust::Trust::Untrusted;
        assert_eq!(
            servers(request.folder(), &sources),
            [stdio("jira", "user-jira", &[]), stdio("notes", "notes-mcp", &[("KEY", "dflt")])],
            "approval alone starts nothing from a folder that is not trusted"
        );
    }

    #[test]
    fn stdio_servers_start_in_the_sandbox_and_only_the_user_s_own_entry_opts_one_out() {
        use crate::mcp::StdioSandbox;
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        write(
            home.path().join(concat!(brand::dir!(), "/mcp.json")),
            r#"{"mcpServers": {
                "mine": {"command": "mine-mcp", "sandbox": false},
                "notes": {"command": "notes-mcp", "sandbox": {"writable": ["~/.local/share/notes"]}}
            }}"#,
        );
        write(
            repo.path().join(concat!(brand::dir!(), "/mcp.json")),
            r#"{"mcpServers": {"theirs": {"command": "theirs-mcp", "sandbox": false}, "bad": {"command": "x", "sandbox": "off"}}}"#,
        );
        let mut sources = McpSources {
            home: Some(home.path().to_path_buf()),
            trust: domain::trust::Trust::Trusted,
            sandbox: Some(domain::sandbox::Sandbox::default()),
            ..McpSources::default()
        };
        sources.approval.enabled = vec!["theirs".into(), "bad".into()];
        let settled = |sources: &McpSources, support: platform::sandbox::Support| -> Vec<(String, StdioSandbox)> {
            configured_on(repo.path(), sources, &support)
                .into_iter()
                .filter_map(|server| match server {
                    ServerSpec::Stdio { name, sandbox, .. } => Some((name, sandbox)),
                    _ => None,
                })
                .collect()
        };
        let found = settled(&sources, platform::sandbox::Support::Available);
        let named = |name: &str| found.iter().find(|(found, _)| found == name).map(|(_, sandbox)| sandbox.clone());
        assert_eq!(named("bad"), None, "a sandbox entry of the wrong shape leaves the server out");
        assert_eq!(named("mine"), Some(StdioSandbox::Unconfined), "the user's own entry may opt out");
        assert!(matches!(named("theirs"), Some(StdioSandbox::Confined(_))), "a project's opt-out is ignored");
        let Some(StdioSandbox::Confined(notes)) = named("notes") else { panic!("{found:?}") };
        assert!(notes.writable.contains(&home.path().join(".local/share/notes")));
        assert!(notes.writable.contains(&repo.path().to_path_buf()) && notes.network);

        sources.sandbox = Some(domain::sandbox::Sandbox { required: true, ..Default::default() });
        let missing = settled(&sources, platform::sandbox::Support::Missing("this kernel has no Landlock".into()));
        assert!(
            missing.iter().any(|(name, sandbox)| name == "theirs" && matches!(sandbox, StdioSandbox::Refused(_))),
            "{missing:?}"
        );
    }

    #[test]
    fn a_project_s_script_server_gets_the_project_s_script_access_and_the_user_s_gets_the_owner_s() {
        use crate::tools::script_tool::{Origin, ScriptAccess};
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let server = "export default { tools: {} };";
        write(repo.path().join(concat!(brand::dir!(), "/mcp/theirs.js")), server);
        write(home.path().join(concat!(brand::dir!(), "/mcp/mine.js")), server);
        let sources = McpSources {
            home: Some(home.path().to_path_buf()),
            trust: domain::trust::Trust::Trusted,
            ..McpSources::default()
        };
        let granted: Vec<(String, ScriptAccess, Origin)> = origins(repo.path(), &sources)
            .into_iter()
            .filter_map(|(_, server)| match server {
                ServerSpec::Script { name, access, origin, .. } => Some((name, access, origin)),
                _ => None,
            })
            .collect();
        assert_eq!(
            granted,
            [
                ("theirs".to_owned(), ScriptAccess::none(), Origin::Project),
                ("mine".to_owned(), ScriptAccess::default(), Origin::Owner),
            ]
        );
    }
}
