pub fn before_start() -> &'static str {
    "Not run: the run was stopped before this call started, so nothing was changed. Run it again if it is still needed."
}

pub fn while_asking(name: &str) -> String {
    format!(
        "Not run: the run was stopped while waiting for the user's answer, so {name} did not run and nothing was changed."
    )
}

pub fn while_running(output: &str) -> String {
    format!(
        "{output}\n\nStopped while running: the command may have run in part or in full, and its changes may have \
         been written. Check what it did before running it again."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_stop_says_whether_anything_may_have_changed() {
        assert!(before_start().contains("nothing was changed"));
        assert!(while_asking("Bash").contains("Bash did not run and nothing was changed"));
        let running = while_running("partial output");
        assert!(running.starts_with("partial output") && running.contains("may have been written"));
    }
}
