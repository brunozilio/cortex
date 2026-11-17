use domain::Command;

use super::app::App;
use super::picker::{Picker, PickerKind};

fn entries(app: &App) -> Vec<(String, String, Command)> {
    let on = |yes: bool| if yes { "on" } else { "off" }.to_owned();
    vec![
        ("Model".into(), app.model().map(|m| m.label.clone()).unwrap_or_default(), Command::Model(None)),
        ("Permission mode".into(), domain::command::mode_label(app.mode()).into(), Command::Mode(None)),
        ("Effort".into(), app.effort_label().into(), Command::Effort(None)),
        (
            "Output style".into(),
            app.session.output_style.clone().unwrap_or_else(|| "default".into()),
            Command::OutputStyle(None),
        ),
        ("Vim keys".into(), on(app.prompt.vim.is_some()), Command::Vim),
        (
            "Status line".into(),
            app.ui.status.command.clone().unwrap_or_else(|| "usual".into()),
            Command::StatusLine(None),
        ),
    ]
}

pub fn open(app: &mut App) {
    let mut items: Vec<(String, String)> = entries(app).into_iter().map(|(name, value, _)| (name, value)).collect();
    items.extend(super::features::settings().into_iter().map(|setting| (setting.name.into(), (setting.value)(app))));
    app.overlays.picker = Some(Picker { kind: PickerKind::Config, title: "Settings".into(), items, cursor: 0 });
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
