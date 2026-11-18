use std::path::{Path, PathBuf};

use crate::api::Reply;

use crate::handler::outbox::Replies;
use crate::services::LocalServices;

fn url_of(services: &LocalServices, path: &Path, name: &str) -> Result<String, String> {
    let Reply::McpServers { servers, .. } =
        crate::mcp_admin::list(path, &services.extensions, services.home().as_deref())
    else {
        return Err("Could not read the MCP servers".into());
    };
    let server =
        servers.iter().find(|server| server.name == name).ok_or_else(|| domain::mcp_config::unknown(name, &servers))?;
    if server.state == domain::mcp_config::McpState::Untrusted {
        return Err(format!("`{name}` is skipped: {}", domain::trust::NOT_TRUSTED));
    }
    if server.target.starts_with("https://") || server.target.starts_with("http://") {
        Ok(server.target.clone())
    } else {
        Err(format!("`{name}` runs on this computer; only remote servers sign in"))
    }
}

pub(crate) fn sign_in(services: &LocalServices, path: PathBuf, name: String, replies: &Replies) {
    let url = match url_of(services, &path, &name) {
        Ok(url) => url,
        Err(error) => return replies.send(Reply::error(error)),
    };
    let (auth, replies) = (services.mcp_auth.clone(), replies.clone());
    let (extensions, home) = (services.extensions.clone(), services.home());
    platform::named::spawn("mcp-sign-in", move || {
        let mut open = |at: &str| replies.send(Reply::McpAuthorizeAt { name: name.clone(), url: at.to_owned() });
        replies.send(match auth.sign_in(&url, replies.stop(), &mut open) {
            Ok(()) => crate::mcp_admin::list(&path, &extensions, home.as_deref()),
            Err(error) => Reply::error(format!("Could not sign in to {name}: {error}")),
        });
    });
}

pub(crate) fn sign_out(services: &LocalServices, path: &Path, name: &str) -> Reply {
    let result = url_of(services, path, name).and_then(|url| services.mcp_auth.sign_out(&url).map_err(String::from));
    match result {
        Ok(true) => crate::mcp_admin::list(path, &services.extensions, services.home().as_deref()),
        Ok(false) => Reply::error(format!("You were not signed in to {name}")),
        Err(error) => Reply::error(error),
    }
}
