use std::path::Path;

use crate::api::Reply;

use crate::extensions::Extensions;

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> Reply {
    let host = extensions.host(path, home);
    match agent_engine::brief::permissions(domain::Folder::at(path, &host)) {
        Ok(permissions) => Reply::Permissions { path: path.to_path_buf(), permissions },
        Err(error) => Reply::error(format!("Could not read the permission rules: {error}")),
    }
}
