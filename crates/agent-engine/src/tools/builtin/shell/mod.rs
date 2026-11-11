mod bash;
mod changes;
mod monitor;
pub mod shells;

use crate::tools::Tool;
use shells::{BashOutput, KillShell};

pub fn tools() -> Vec<Box<dyn Tool>> {
    vec![Box::new(bash::Bash), Box::new(BashOutput), Box::new(KillShell), Box::new(monitor::Monitor)]
}

fn clip_middle(text: &str, limit: usize) -> String {
    let count = text.chars().count();
    if count <= limit {
        return text.to_owned();
    }
    let head: String = text.chars().take(limit / 2).collect();
    let tail: String = text.chars().skip(count - (limit - limit / 2)).collect();
    format!("{head}\n… [{} characters cut from the middle] …\n{tail}", count - limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_output_keeps_its_start_and_its_end() {
        assert_eq!(clip_middle("abcdef", 6), "abcdef");
        assert_eq!(clip_middle("ábcdefghij", 4), "áb\n… [6 characters cut from the middle] …\nij");
    }
}
