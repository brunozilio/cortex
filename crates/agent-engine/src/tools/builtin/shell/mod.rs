pub mod affected;
mod bash;
mod changes;
mod git;
mod monitor;
pub mod savings;
mod shaped;
pub mod shells;
mod snapshot;

use crate::tools::Tool;
use shells::{BashOutput, KillShell};

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(bash::Bash), Box::new(BashOutput), Box::new(KillShell), Box::new(monitor::Monitor)]
}
