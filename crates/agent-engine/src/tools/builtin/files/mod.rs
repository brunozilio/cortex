mod edit;
mod glob;
mod grep;
mod notebook;
mod pdf;
mod read;
mod shown_diff;
mod write;

use crate::tools::Tool;

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
