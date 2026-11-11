use domain::Command;

use super::app::App;
use super::choices::{Picker, PickerKind};

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
    ]
}

pub fn open(app: &mut App) {
    let mut items: Vec<(String, String)> = entries(app).into_iter().map(|(name, value, _)| (name, value)).collect();
    items.extend(super::features::settings().into_iter().map(|setting| (setting.name.into(), (setting.value)(app))));
    app.picker = Some(Picker { kind: PickerKind::Config, title: "Settings".into(), items, cursor: 0 });
}

pub fn picked(app: &mut App, cursor: usize) {
    let entries = entries(app);
    let count = entries.len();
    if let Some((_, _, command)) = entries.into_iter().nth(cursor) {
        return super::commands::run(app, command);
    }
    if let Some(setting) = super::features::settings().into_iter().nth(cursor - count) {
        (setting.run)(app, None);
    }
}

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
