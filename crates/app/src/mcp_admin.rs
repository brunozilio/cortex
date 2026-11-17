use std::path::Path;

use crate::api::Reply;
use domain::mcp_config;
use domain::mcp_config::{McpEntry, McpListing, McpOrigin};
use platform::store::Document;
use serde_json::Value;

use crate::extensions::Extensions;

fn listing(path: &Path, extensions: &Extensions, home: Option<&Path>) -> Result<McpListing, String> {
    let host = extensions.host(path, home);
    agent_engine::mcp::listing(domain::Folder::at(path, &host))
        .map_err(|error| format!("Could not read the MCP servers: {error}"))
}

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> Reply {
    match listing(path, extensions, home) {
        Ok(McpListing { servers, unreadable }) => Reply::McpServers { path: path.to_path_buf(), servers, unreadable },
        Err(error) => Reply::error(error),
    }
}

pub(crate) fn known(path: &Path, extensions: &Extensions, home: Option<&Path>, name: &str) -> Result<(), String> {
    listing(path, extensions, home)?.named(name).map(|_| ())
}

pub(crate) fn get(path: &Path, extensions: &Extensions, home: Option<&Path>, name: &str) -> Reply {
    let found = listing(path, extensions, home).and_then(|listing| listing.named(name).cloned());
    let info = match found {
        Ok(info) => info,
        Err(error) => return Reply::error(error),
    };
    let file = match info.origin {
        McpOrigin::User => home.map(|home| home.join(brand::DIR).join("mcp.json")),
        McpOrigin::Project => agent_engine::mcp::project_config(path, home),
        McpOrigin::Host | McpOrigin::Plugin(_) => None,
    };
    let entry = file.as_deref().and_then(|file| stored(file, name));
    let file = entry.is_some().then_some(file).flatten();
    Reply::McpServer(McpEntry { info, file, entry: entry.map(|entry| mcp_config::redacted(&entry)) })
}

fn stored(file: &Path, name: &str) -> Option<Value> {
    let config: Value = serde_json::from_str(&std::fs::read_to_string(file).ok()?).ok()?;
    config["mcpServers"].get(name).cloned()
}

pub(crate) fn approve(path: &Path, home: Option<&Path>, name: &str, approve: bool) -> Result<(), String> {
    let config = agent_engine::mcp::project_config(path, home);
    let folder = config.as_deref().and_then(Path::parent).unwrap_or(path);
    let file = if config.is_some() {
        folder.join("settings.local.json")
    } else {
        folder.join(concat!(brand::dir!(), "/settings.local.json"))
    };
    rewrite(&Document::hand_edited(file), |settings| Ok(mcp_config::set_approval(settings, name, approve)))
}

pub(crate) fn add(home: Option<&Path>, name: &str, server: Value, replace: bool) -> Result<(), String> {
    rewrite(&user_config(home)?, |config| mcp_config::add_server(config, name, server, replace))
}

pub(crate) fn remove(home: Option<&Path>, name: &str) -> Result<(), String> {
    rewrite(&user_config(home)?, |config| match mcp_config::remove_server(config, name) {
        (config, true) => Ok(config),
        (_, false) => Err(format!("`{name}` is not one of your MCP servers")),
    })
}

fn user_config(home: Option<&Path>) -> Result<Document, String> {
    let home = home.ok_or_else(|| "no home folder to keep it in".to_owned())?;
    Ok(Document::at(home.join(brand::DIR).join("mcp.json")).authored())
}

fn rewrite(file: &Document, change: impl FnOnce(Value) -> Result<Value, String>) -> Result<(), String> {
    let rewritten = file.try_update(|current: &mut Value| {
        *current = change(std::mem::take(current))?;
        Ok::<(), String>(())
    });
    rewritten.map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use domain::mcp_config::{McpOrigin, McpState};
    use serde_json::json;

    use super::*;

    fn states(event: Reply) -> Vec<(String, McpOrigin, McpState)> {
        let Reply::McpServers { servers, .. } = event else { panic!("not a listing: {event:?}") };
        servers.into_iter().map(|s| (s.name, s.origin, s.state)).collect()
    }

    #[test]
    fn the_owner_approves_a_project_s_server_and_keeps_their_own() {
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(brand::DIR)).expect("dir");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/mcp.json")),
            r#"{"mcpServers": {"jira": {"command": "jira-mcp"}}}"#,
        )
        .expect("config");
        let cwd = repo.path().join("src");
        std::fs::create_dir_all(&cwd).expect("cwd");
        let host = Extensions::default();
        let list = || states(list(&cwd, &host, Some(home.path())));
        assert_eq!(
            list(),
            [("jira".into(), McpOrigin::Project, McpState::Untrusted)],
            "an untrusted folder's servers are shown as skipped"
        );
        assert!(known(&cwd, &host, Some(home.path()), "jira").is_ok());
        assert_eq!(
            known(&cwd, &host, Some(home.path()), "ghost"),
            Err("No MCP server `ghost` here; known: jira".into())
        );
        std::fs::create_dir_all(repo.path().join(".git")).expect("a repository, so src is inside the project");
        host.trusts.set(&cwd, Some(home.path()), true).expect("trusted");
        assert_eq!(list(), [("jira".into(), McpOrigin::Project, McpState::Pending)]);

        approve(&cwd, Some(home.path()), "jira", true).expect("approve");
        assert!(
            repo.path().join(concat!(brand::dir!(), "/settings.local.json")).is_file(),
            "kept beside the project's mcp.json"
        );
        assert_eq!(list()[0].2, McpState::Ready);

        let atlassian = json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"});
        add(Some(home.path()), "atlassian", atlassian, false).expect("add");
        assert_eq!(list()[1], ("atlassian".into(), McpOrigin::User, McpState::Ready));
        let reserved = concat!(brand::command!(), "_memory");
        assert!(add(Some(home.path()), reserved, json!({"command": "x"}), false).is_err());
        let Reply::McpServer(shown) = get(&cwd, &host, Some(home.path()), "atlassian") else { panic!("not shown") };
        assert_eq!(shown.file, Some(home.path().join(brand::DIR).join("mcp.json")));
        assert_eq!(shown.entry, Some(json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"})));
        assert!(matches!(get(&cwd, &host, Some(home.path()), "ghost"), Reply::Error { .. }));

        remove(Some(home.path()), "atlassian").expect("remove");
        assert_eq!(list().len(), 1);
        assert!(remove(Some(home.path()), "atlassian").is_err(), "nothing left to remove");
    }

    #[test]
    fn a_config_its_author_left_unparsable_is_reported_and_stays_theirs() {
        let home = tempfile::tempdir().expect("home");
        let config = home.path().join(brand::DIR).join("mcp.json");
        std::fs::create_dir_all(config.parent().expect("folder")).expect("dir");
        std::fs::write(&config, "{\"mcpServers\": {").expect("config");

        let refused = add(Some(home.path()), "atlassian", json!({"command": "x"}), false).expect_err("refused");

        assert!(refused.contains("mcp.json"), "{refused}");
        assert_eq!(std::fs::read_to_string(&config).expect("read"), "{\"mcpServers\": {");
    }

    #[cfg(unix)]
    #[test]
    fn the_owner_s_servers_are_theirs_alone_and_a_project_s_approvals_stay_as_shared_as_they_were() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).expect("metadata").permissions().mode() & 0o777;
        let home = tempfile::tempdir().expect("home");
        let repo = tempfile::tempdir().expect("repo");
        let local = repo.path().join(concat!(brand::dir!(), "/settings.local.json"));
        std::fs::create_dir_all(local.parent().expect("folder")).expect("dir");
        std::fs::write(&local, "{\"permissions\": {\"deny\": [\"Bash(rm:*)\"]}}").expect("local");
        std::fs::set_permissions(&local, std::fs::Permissions::from_mode(0o644)).expect("mode");

        add(Some(home.path()), "atlassian", json!({"command": "x"}), false).expect("add");
        approve(repo.path(), Some(home.path()), "jira", true).expect("approve");

        assert_eq!(mode(&home.path().join(brand::DIR).join("mcp.json")), 0o600);
        assert_eq!(mode(&local), 0o644);
        let kept: Value = serde_json::from_str(&std::fs::read_to_string(&local).expect("read")).expect("json");
        assert_eq!(kept["permissions"]["deny"], json!(["Bash(rm:*)"]), "what was there stays");
        assert_eq!(std::fs::read_dir(local.parent().expect("folder")).expect("list").count(), 1);
    }
}
