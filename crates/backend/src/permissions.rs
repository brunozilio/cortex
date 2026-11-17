use std::path::Path;

use domain::{ModelOption, PromptRequest};
use protocol::Reply;

use crate::extensions::Extensions;

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> Reply {
    let mut request = PromptRequest::plain(path.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(domain::Extensions {
        plugins: extensions.plugins.clone(),
        home: home.map(Path::to_path_buf),
        ..Default::default()
    });
    match agent_engine::brief::permissions(&request) {
        Ok(permissions) => Reply::Permissions { path: path.to_path_buf(), permissions },
        Err(error) => Reply::error(format!("Could not read the permission rules: {error}")),
    }
}
