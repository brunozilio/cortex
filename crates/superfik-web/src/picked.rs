//! Files the visitor picked in the browser, held until they are sent. A
//! tab has no paths: each file is known by `picked/<n>/<name>`, which the
//! reducer tracks like a dropped file's path until the server says where
//! it keeps it (`ServerEvent::Uploaded`). Pure.

use std::collections::HashMap;
use std::path::PathBuf;

use superfik_protocol::ClientCommand;

#[derive(Debug, Default)]
pub struct Picked {
    next: u32,
    files: HashMap<PathBuf, (String, Vec<u8>)>,
}

impl Picked {
    pub fn add(&mut self, files: Vec<(String, Vec<u8>)>) -> Vec<PathBuf> {
        files
            .into_iter()
            .map(|(name, bytes)| {
                self.next += 1;
                let known = PathBuf::from(format!("picked/{}/{name}", self.next));
                self.files.insert(known.clone(), (name, bytes));
                known
            })
            .collect()
    }

    pub fn uploads(&mut self, paths: Vec<PathBuf>) -> Vec<Result<ClientCommand, String>> {
        paths
            .into_iter()
            .filter_map(|path| {
                let (name, bytes) = self.files.remove(&path)?;
                Some(superfik_client::upload::command(path, name, &bytes))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picked_file_is_sent_once_by_the_name_it_was_given() {
        let mut picked = Picked::default();
        let known = picked.add(vec![("sky.png".into(), b"sky".to_vec()), ("sky.png".into(), b"sea".to_vec())]);
        assert_ne!(known[0], known[1], "two files of one name stay apart");
        let sent = picked.uploads(vec![known[0].clone()]);
        assert!(matches!(&sent[..], [Ok(ClientCommand::Upload { from, name, data })]
            if *from == known[0] && name == "sky.png" && data == "c2t5"));
        assert!(picked.uploads(vec![known[0].clone()]).is_empty(), "sent once");
    }
}
