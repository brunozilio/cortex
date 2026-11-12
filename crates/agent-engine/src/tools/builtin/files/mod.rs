mod edit;
mod glob;
mod grep;
mod notebook;
mod pdf;
mod read;
mod shown_diff;
mod write;

use crate::tools::Tool;

fn changed_since_read(name: &str) -> String {
    format!(
        "{name} changed on disk since you last read it (the user, a formatter or another tool wrote to it). Read it \
         again before changing it, so your change does not overwrite theirs."
    )
}

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(read::Read),
        Box::new(write::Write),
        Box::new(edit::Edit),
        Box::new(glob::Glob),
        Box::new(grep::Grep),
        Box::new(notebook::NotebookEdit),
    ]
}
