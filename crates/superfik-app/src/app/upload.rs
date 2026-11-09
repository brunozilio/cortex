//! Reads a file of this computer into the command that sends it to an
//! organization's server (`superfik_client::upload`, ADR 0010).

use std::path::Path;

use superfik_client::upload::AT_MOST;
use superfik_protocol::ClientCommand;

pub fn command(path: &Path) -> Result<ClientCommand, String> {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let size = std::fs::metadata(path).map_err(|error| format!("could not read {name}: {error}"))?.len();
    if size > AT_MOST as u64 {
        return Err(format!("{name} is over {} MB, too large to send", AT_MOST / (1024 * 1024)));
    }
    let bytes = std::fs::read(path).map_err(|error| format!("could not read {name}: {error}"))?;
    superfik_client::upload::command(path.to_path_buf(), name, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_on_disk_becomes_its_name_and_contents() {
        let folder = tempfile::tempdir().expect("folder");
        let path = folder.path().join("sky.png");
        std::fs::write(&path, b"sky").expect("write");
        let ClientCommand::Upload { from, name, data } = command(&path).expect("command") else { panic!("not upload") };
        assert_eq!((from, name, data.as_str()), (path, "sky.png".to_owned(), "c2t5"));
        assert!(command(&folder.path().join("missing")).is_err());
    }
}
