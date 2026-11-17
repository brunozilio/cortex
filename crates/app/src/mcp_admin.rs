use std::path::Path;

use crate::api::Reply;
use domain::mcp_config;
use domain::{ModelOption, PromptRequest};
use platform::store::Document;
use serde_json::Value;

use crate::extensions::Extensions;

pub(crate) fn request_in(path: &Path, extensions: &Extensions, home: Option<&Path>) -> PromptRequest {
    let mut request = PromptRequest::plain(path.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(domain::Extensions {
        plugins: extensions.plugins.clone(),
        home: home.map(Path::to_path_buf),
        ..Default::default()
    });
    request
}

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> Reply {
    let request = request_in(path, extensions, home);
    match agent_engine::mcp::list(&request) {
        Ok(servers) => Reply::McpServers { path: path.to_path_buf(), servers },
        Err(error) => Reply::error(format!("Could not read the MCP servers: {error}")),
    }
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

pub(crate) fn add(home: Option<&Path>, name: &str, server: Value) -> Result<(), String> {
    rewrite(&user_config(home)?, |config| mcp_config::add_server(config, name, server))
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
        assert_eq!(list(), [("jira".into(), McpOrigin::Project, McpState::Pending)]);

        approve(&cwd, Some(home.path()), "jira", true).expect("approve");
        assert!(
            repo.path().join(concat!(brand::dir!(), "/settings.local.json")).is_file(),
            "kept beside the project's mcp.json"
        );
        assert_eq!(list()[0].2, McpState::Ready);

        let atlassian = json!({"type": "http", "url": "https://mcp.atlassian.com/v1/mcp"});
        add(Some(home.path()), "atlassian", atlassian).expect("add");
        assert_eq!(list()[1], ("atlassian".into(), McpOrigin::User, McpState::Ready));
        assert!(add(Some(home.path()), concat!(brand::command!(), "_memory"), json!({"command": "x"})).is_err());

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

        let refused = add(Some(home.path()), "atlassian", json!({"command": "x"})).expect_err("refused");

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

        add(Some(home.path()), "atlassian", json!({"command": "x"})).expect("add");
        approve(repo.path(), Some(home.path()), "jira", true).expect("approve");

        assert_eq!(mode(&home.path().join(brand::DIR).join("mcp.json")), 0o600);
        assert_eq!(mode(&local), 0o644);
        let kept: Value = serde_json::from_str(&std::fs::read_to_string(&local).expect("read")).expect("json");
        assert_eq!(kept["permissions"]["deny"], json!(["Bash(rm:*)"]), "what was there stays");
        assert_eq!(std::fs::read_dir(local.parent().expect("folder")).expect("list").count(), 1);
    }
}
