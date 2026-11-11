use domain::command::{Command, closest, slash_name};

use super::app::App;

fn builtin() -> impl Iterator<Item = (&'static str, &'static str)> {
    Command::HELP.into_iter().chain(super::features::help())
}

fn about(name: &str) -> Option<(&'static str, &'static str)> {
    builtin().find(|(known, _)| known.eq_ignore_ascii_case(name))
}

pub fn help(app: &mut App, name: Option<String>, keys: &str) {
    let Some(name) = name else {
        let mut text = String::from("Commands:");
        for (name, about) in builtin() {
            text.push_str(&format!("\n  /{name:<12} {about}"));
        }
        text.push_str("\n  /<skill>      Run a skill or custom command\n");
        text.push_str(keys);
        return app.transcript.notice(text);
    };
    let text = match (about(&name), app.skills.iter().find(|(skill, _)| skill.eq_ignore_ascii_case(&name))) {
        (Some((name, about)), _) => format!("/{name}  {about}"),
        (None, Some((skill, about))) => format!("/{skill}  {about}"),
        (None, None) => unknown(app, &name),
    };
    app.transcript.notice(text);
}

pub fn refused(app: &mut App, text: &str) -> bool {
    let Some(name) = slash_name(text) else { return false };
    let skills_known = !app.skills.is_empty();
    let is_skill = app.skills.iter().any(|(skill, _)| skill.eq_ignore_ascii_case(name));
    let notice = if Command::parse(&format!("/{name}")).is_some() {
        match about(name) {
            Some((name, about)) => format!("/{name} can't take that. {about}"),
            None => format!("/{name} can't take that. /help lists how each command is used."),
        }
    } else if skills_known && !is_skill && !name.starts_with("mcp__") {
        unknown(app, name)
    } else {
        return false;
    };
    app.input.set(text);
    app.transcript.notice(notice);
    true
}

fn unknown(app: &App, name: &str) -> String {
    let known = builtin().map(|(name, _)| name).chain(app.skills.iter().map(|(skill, _)| skill.as_str()));
    match closest(name, known) {
        Some(near) => format!("/{name} is not a command or skill. Did you mean /{near}?"),
        None => format!("/{name} is not a command or skill. /help lists them."),
    }
}
