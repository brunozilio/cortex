//! `/permissions`: the rules and added folders of a folder's settings, as
//! the engine reads them for a run there.

use std::path::Path;

use superfik_core::{ModelOption, PromptRequest};
use superfik_protocol::ServerEvent;

use crate::extensions::Extensions;

pub(crate) fn list(path: &Path, extensions: &Extensions, home: Option<&Path>) -> ServerEvent {
    let mut request = PromptRequest::plain(path.to_path_buf(), "", ModelOption::new("", ""));
    request.extensions = Some(superfik_core::Extensions {
        plugins: extensions.plugins.clone(),
        home: home.map(Path::to_path_buf),
        ..Default::default()
    });
    match superfik_native::brief::permissions(&request) {
        Ok(permissions) => ServerEvent::Permissions { path: path.to_path_buf(), permissions },
        Err(error) => ServerEvent::error(format!("Could not read the permission rules: {error}")),
    }
}
