use serde_json::{Value, json};

use super::{Feature, Setting, Slash};
use crate::tui::app::{App, Msg};

const NAME: &str = "flags";

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["features"],
            about: "List the features you can turn off, or switch one: /features <name> on|off",
            run,
        }],
        setting: Some(Setting { name: "Features", value: |_| String::new(), run }),
        ..Feature::default()
    }
}

fn run(app: &mut App, argument: Option<String>) {
    let toggle = match domain::features::toggle(argument.as_deref()) {
        Ok(toggle) => toggle,
        Err(error) => return app.transcript.notice(error),
    };
    let mut input = json!({ "cwd": app.cwd });
    if let Some((name, on)) = &toggle {
        input["name"] = json!(name);
        input["on"] = json!(on);
    }
    app.spawn(move |sdk| match sdk.feature(NAME, input) {
        Ok(listed) => Msg::Notice(shown(&listed, toggle.is_some())),
        Err(error) => Msg::Error(error.to_string()),
    });
}

fn shown(listed: &Value, changed: bool) -> String {
    let lines = listed.as_array().into_iter().flatten().map(|flag| {
        let state = if flag["on"] == true { "on " } else { "off" };
        let unavailable = if flag["available"] == false { " (not on this system)" } else { "" };
        format!(
            "  {state}  {}{unavailable}\n       {}",
            flag["name"].as_str().unwrap_or_default(),
            flag["about"].as_str().unwrap_or_default()
        )
    });
    let head = if changed { "Saved in your settings; new runs follow it.\n" } else { "" };
    format!("{head}Features (/features <name> on|off):\n{}", lines.collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_flag_shows_its_state_and_whether_this_system_has_it() {
        let listed = json!([
            { "name": "experimental-sandbox", "about": "Confine commands", "on": false, "available": false },
            { "name": "budget", "about": "Spend limit", "on": true, "available": true }
        ]);

        let text = shown(&listed, true);

        assert!(text.starts_with("Saved in your settings"));
        assert!(text.contains("  off  experimental-sandbox (not on this system)\n       Confine commands"));
        assert!(text.contains("  on   budget\n       Spend limit"));
    }
}
