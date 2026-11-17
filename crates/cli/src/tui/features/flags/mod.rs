use std::collections::BTreeSet;

use app::ListedFlag;

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

fn load_once(app: &mut App) {
    let Some(loaded) = app.features.get::<Loaded>(NAME) else { return };
    if std::mem::replace(&mut loaded.0, true) {
        return;
    }
    let cwd = app.cwd.clone();
    app.spawn(move |client| match client.flags(&cwd) {
        Ok(listed) => remember(&listed),
        Err(_) => Msg::Nothing,
    });
}

fn remember(listed: &[ListedFlag]) -> Msg {
    let off = turned_off(listed);
    super::reply(move |app| app.session.turned_off = off)
}

fn turned_off(listed: &[ListedFlag]) -> BTreeSet<String> {
    listed.iter().filter(|listed| !listed.flag.on).map(|listed| listed.flag.name.clone()).collect()
}

fn run(app: &mut App, argument: Option<String>) {
    let toggle = match domain::features::toggle(argument.as_deref()) {
        Ok(toggle) => toggle,
        Err(error) => return app.transcript.notice(error),
    };
    let cwd = app.cwd.clone();
    app.spawn(move |client| {
        let listed = match &toggle {
            Some((name, on)) => client.set_flag(&cwd, name, *on),
            None => client.flags(&cwd),
        };
        match listed {
            Ok(listed) => {
                let (off, text) = (turned_off(&listed), shown(&listed, toggle.is_some()));
                super::reply(move |app| {
                    app.session.turned_off = off;
                    app.transcript.notice(text);
                })
            }
            Err(error) => Msg::Error(error.to_string()),
        }
    });
}

fn shown(listed: &[ListedFlag], changed: bool) -> String {
    let lines = listed.iter().map(|ListedFlag { flag, available }| {
        let state = if flag.on { "on " } else { "off" };
        let unavailable = if *available { "" } else { " (not on this system)" };
        format!("  {state}  {}{unavailable}\n       {}", flag.name, flag.about)
    });
    let head = if changed { "Saved in your settings; new runs follow it.\n" } else { "" };
    format!("{head}Features (/features <name> on|off):\n{}", lines.collect::<Vec<_>>().join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(name: &str, about: &str, on: bool, available: bool) -> ListedFlag {
        let flag = domain::features::Listed {
            name: name.into(),
            about: about.into(),
            on,
            experimental: name.starts_with("experimental-"),
            security: false,
        };
        ListedFlag { flag, available }
    }

    #[test]
    fn each_flag_shows_its_state_and_whether_this_system_has_it() {
        let listed =
            [flag("experimental-sandbox", "Confine commands", false, false), flag("budget", "Spend limit", true, true)];

        let text = shown(&listed, true);

        assert!(text.starts_with("Saved in your settings"));
        assert!(text.contains("  off  experimental-sandbox (not on this system)\n       Confine commands"));
        assert!(text.contains("  on   budget\n       Spend limit"));
        assert_eq!(turned_off(&listed), BTreeSet::from(["experimental-sandbox".to_owned()]));
    }
}
