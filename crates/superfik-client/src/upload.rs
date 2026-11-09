//! A file of this client turned into the command that sends it to the
//! server (`Effect::Upload`, ADR 0010: the server cannot read the client's
//! files). The desktop reads it from disk, the web from a file picker;
//! both send it this way. Pure.

use std::path::PathBuf;

use base64::Engine;
use superfik_protocol::ClientCommand;

pub const AT_MOST: usize = 25 * 1024 * 1024;

pub fn command(from: PathBuf, name: String, bytes: &[u8]) -> Result<ClientCommand, String> {
    if bytes.len() > AT_MOST {
        return Err(format!("{name} is over {} MB, too large to send", AT_MOST / (1024 * 1024)));
    }
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(ClientCommand::Upload { from, name, data })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_becomes_its_name_and_contents() {
        let command = command("/a/sky.png".into(), "sky.png".into(), b"sky").expect("command");
        let ClientCommand::Upload { from, name, data } = command else { unreachable!("an upload") };
        assert_eq!((from, name, data.as_str()), ("/a/sky.png".into(), "sky.png".to_owned(), "c2t5"));
    }

    #[test]
    fn too_large_a_file_is_refused_before_it_is_sent() {
        let big = vec![0; AT_MOST + 1];
        assert!(command("big.bin".into(), "big.bin".into(), &big).is_err());
    }
}
