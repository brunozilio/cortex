//! `/config`: the chat's settings in one list, each opening its own choice;
//! `/agents new` and `/skills new`, which ask the agent to write one; and
//! `/memory edit`, which opens the project's instructions in `$EDITOR`.

use std::path::PathBuf;

use domain::Command;

use super::app::App;
use super::choices::{Picker, PickerKind};

/// What `/config` lists, and the command each runs.
fn entries(app: &App) -> Vec<(String, String, Command)> {
    let on = |yes: bool| if yes { "on" } else { "off" }.to_owned();
    vec![
        ("Model".into(), app.model().map(|m| m.label.clone()).unwrap_or_default(), Command::Model(None)),
        ("Permission mode".into(), domain::command::mode_label(app.mode()).into(), Command::Mode(None)),
        ("Effort".into(), app.effort().cli_value().into(), Command::Effort(None)),
        (
            "Output style".into(),
            app.output_style.clone().unwrap_or_else(|| "default".into()),
            Command::OutputStyle(None),
        ),
        ("Vim keys".into(), on(app.vim.is_some()), Command::Vim),
        ("Mouse wheel scrolls".into(), on(app.mouse), Command::Mouse),
        ("Status line".into(), app.status.command.clone().unwrap_or_else(|| "usual".into()), Command::StatusLine(None)),
        ("Verification".into(), String::new(), Command::Verification(domain::command::VerificationCommand::Show)),
    ]
}

pub fn open(app: &mut App) {
    let items = entries(app).into_iter().map(|(name, value, _)| (name, value)).collect();
    app.picker = Some(Picker { kind: PickerKind::Config, title: "Settings".into(), items, cursor: 0 });
}

pub fn picked(app: &mut App, cursor: usize) {
    if let Some((_, _, command)) = entries(app).into_iter().nth(cursor) {
        super::commands::run(app, command);
    }
}

/// Asks the agent to write a sub-agent or a skill for `what`.
pub fn create(app: &mut App, agent: bool, what: &str) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish first.");
    }
    let prompt = if agent {
        format!(
            "Create a {brand_name} sub-agent for this: {what}\n\nWrite it to {brand_dir}/agents/<name>.md: YAML \
             frontmatter with `name` (lowercase, hyphens), `description` (when to use it, one or two sentences), \
             and `tools` only if it should have fewer than all; then its instructions as the body. Ask me first \
             if what it should do is unclear.",
            brand_name = brand::NAME,
            brand_dir = brand::DIR
        )
    } else {
        format!(
            "Create a {brand_name} skill for this: {what}\n\nLoad the skill-creator skill and follow it. Write the \
             skill under {brand_dir}/skills/<name>/SKILL.md.",
            brand_name = brand::NAME,
            brand_dir = brand::DIR
        )
    };
    app.send(prompt);
}

/// The project's instructions file: <COMMAND>.md at its root, else
/// .<command>/<COMMAND>.md, else a new <COMMAND>.md.
pub fn instructions_file(cwd: &std::path::Path) -> PathBuf {
    let root = cwd.join(brand::INSTRUCTIONS);
    let nested = cwd.join(brand::DIR).join(brand::INSTRUCTIONS);
    if !root.exists() && nested.exists() { nested } else { root }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_instructions_file_is_the_one_the_project_has() {
        let dir = tempfile::tempdir().expect("dir");
        assert_eq!(super::instructions_file(dir.path()), dir.path().join(brand::INSTRUCTIONS));
        std::fs::create_dir_all(dir.path().join(brand::DIR)).expect("dir");
        std::fs::write(dir.path().join(concat!(brand::dir!(), "/", brand::instructions!())), "x").expect("write");
        assert_eq!(
            super::instructions_file(dir.path()),
            dir.path().join(concat!(brand::dir!(), "/", brand::instructions!()))
        );
    }
}
