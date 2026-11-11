mod bash;
mod monitor;
pub mod shells;

use crate::tools::Tool;
use shells::{BashOutput, KillShell};

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(bash::Bash), Box::new(BashOutput), Box::new(KillShell), Box::new(monitor::Monitor)]
}
