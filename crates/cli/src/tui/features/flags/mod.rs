use std::collections::BTreeSet;

use serde_json::{Value, json};

use super::{Feature, Setting, Slash};
use crate::tui::app::{App, Msg};

const NAME: &str = "flags";

#[derive(Default)]
struct Loaded(bool);

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["features"],
            about: "List the features you can turn off, or switch one: /features <name> on|off",
            run,
        }],
        tick: Some(load_once),
        setting: Some(Setting { name: "Features", value: |_| String::new(), run }),
        ..Feature::default()
    }
}

pub fn on(app: &App, name: &str) -> bool {
    !app.turned_off.contains(name)
}

fn load_once(app: &mut App) {
    let Some(loaded) = app.features.get::<Loaded>(NAME) else { return };
    if std::mem::replace(&mut loaded.0, true) {
        return;
    }
    let input = json!({ "cwd": app.cwd });
    app.spawn(move |sdk| match sdk.feature(NAME, input) {
        Ok(listed) => remember(&listed),
        Err(_) => Msg::Feature(Box::new(|_| {})),
    });
}

fn remember(listed: &Value) -> Msg {
    let off = turned_off(listed);
    Msg::Feature(Box::new(move |app| app.turned_off = off))
}

fn turned_off(listed: &Value) -> BTreeSet<String> {
    let flags = listed.as_array().into_iter().flatten();
    flags.filter(|flag| flag["on"] == false).filter_map(|flag| flag["name"].as_str().map(str::to_owned)).collect()
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
        Ok(listed) => {
            let (off, text) = (turned_off(&listed), shown(&listed, toggle.is_some()));
            Msg::Feature(Box::new(move |app| {
                app.turned_off = off;
                app.transcript.notice(text);
            }))
        }
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
        assert_eq!(turned_off(&listed), BTreeSet::from(["experimental-sandbox".to_owned()]));
    }
}
