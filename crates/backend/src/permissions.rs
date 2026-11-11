use std::path::Path;

use domain::{ModelOption, PromptRequest};
use protocol::ServerEvent;

use crate::extensions::Extensions;

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> ServerEvent {
    let mut request = PromptRequest::plain(path.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(domain::Extensions {
        plugins: extensions.plugins.clone(),
        home: home.map(Path::to_path_buf),
        ..Default::default()
    });
    match agent_engine::brief::permissions(&request) {
        Ok(permissions) => ServerEvent::Permissions { path: path.to_path_buf(), permissions },
        Err(error) => ServerEvent::error(format!("Could not read the permission rules: {error}")),
    }
}
