//! `SignInMcpServer` and `SignOutMcpServer`: a server named as the folder's
//! listing names it, signed in to on a thread of its own (the browser may
//! take minutes), answered with the listing again.

use std::path::{Path, PathBuf};

use superfik_core::Principal;
use superfik_protocol::ServerEvent;

use crate::outbox::Outbox;
use crate::services::LocalServices;

/// The URL of the remote server `name` a run in `path` can use.
fn url_of(services: &LocalServices, path: &Path, name: &str) -> Result<String, String> {
    let ServerEvent::McpServers { servers, .. } =
        crate::mcp_admin::list(path, &services.extensions, services.home.as_deref())
    else {
        return Err("Could not read the MCP servers".into());
    };
    let server =
        servers.iter().find(|server| server.name == name).ok_or_else(|| format!("No MCP server `{name}` here"))?;
    if server.target.starts_with("https://") || server.target.starts_with("http://") {
        Ok(server.target.clone())
    } else {
        Err(format!("`{name}` runs on this computer; only remote servers sign in"))
    }
}

pub(crate) fn sign_in(services: &LocalServices, owner: &Principal, path: PathBuf, name: String, outbox: &Outbox) {
    let url = match url_of(services, &path, &name) {
        Ok(url) => url,
        Err(error) => return outbox.send(ServerEvent::error(error)),
    };
    let (auth, owner, outbox) = (services.mcp_auth.clone(), owner.clone(), outbox.clone());
    let (extensions, home) = (services.extensions.clone(), services.home.clone());
    std::thread::spawn(move || {
        let mut open = |at: &str| outbox.send(ServerEvent::McpAuthorizeAt { name: name.clone(), url: at.to_owned() });
        match auth.sign_in(&owner, &url, &mut open) {
            Ok(()) => outbox.send(crate::mcp_admin::list(&path, &extensions, home.as_deref())),
            Err(error) => outbox.send(ServerEvent::error(format!("Could not sign in to {name}: {error}"))),
        }
    });
}

pub(crate) fn sign_out(services: &LocalServices, owner: &Principal, path: &Path, name: &str, outbox: &Outbox) {
    let result = url_of(services, path, name).and_then(|url| services.mcp_auth.sign_out(owner, &url));
    outbox.send(match result {
        Ok(true) => crate::mcp_admin::list(path, &services.extensions, services.home.as_deref()),
        Ok(false) => ServerEvent::error(format!("You were not signed in to {name}")),
        Err(error) => ServerEvent::error(error),
    });
}
