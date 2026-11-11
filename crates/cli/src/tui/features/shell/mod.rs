use serde_json::json;

use super::{Feature, Slash};
use crate::tui::app::{App, Msg};

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["bashes"],
            about: "List commands left in the background, /bashes kill <id> stops one",
            run: bashes,
        }],
        tick: None,
    }
}

fn bashes(app: &mut App, argument: Option<String>) {
    let kill = match argument.as_deref().map(|a| a.split_once(' ').unwrap_or((a, ""))) {
        None => None,
        Some(("kill", id)) if !id.trim().is_empty() => Some(id.trim().to_owned()),
        Some(_) => return app.transcript.notice("/bashes lists them; /bashes kill <id> stops one."),
    };
    app.spawn(move |sdk| match sdk.feature("shell", json!({ "kill": kill })) {
        Ok(found) => Msg::Notice(listed(&found["shells"])),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn listed(shells: &serde_json::Value) -> String {
    let shells: Vec<(String, String, bool)> = serde_json::from_value(shells.clone()).unwrap_or_default();
    if shells.is_empty() {
        return "No commands in the background.".into();
    }
    let lines: Vec<String> = shells
        .iter()
        .map(|(id, command, running)| format!("  {id:<10} {} {command}", if *running { "running " } else { "finished" }))
        .collect();
    format!("In the background (/bashes kill <id> stops one):\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_and_finished_commands_are_listed() {
        assert_eq!(listed(&json!([])), "No commands in the background.");
        let shown = listed(&json!([["bash_1", "sleep 9", true], ["bash_2", "ls", false]]));
        assert!(shown.contains("bash_1     running  sleep 9") && shown.contains("bash_2     finished ls"), "{shown}");
    }
}
