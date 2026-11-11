mod bash;
mod monitor;

use crate::tools::Tool;
use crate::tools::shells::{BashOutput, KillShell};

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(bash::Bash), Box::new(BashOutput), Box::new(KillShell), Box::new(monitor::Monitor)]
}
