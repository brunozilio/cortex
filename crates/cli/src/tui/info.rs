use domain::Command;
use domain::command::mode_label;
use domain::provider::ModelProvider;

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};

const RELEASE_NOTES: &str = include_str!(concat!(env!("OUT_DIR"), "/release_notes.md"));

pub fn run(app: &mut App, command: Command) {
    match command {
        Command::Status => {
            let text = status(app);
            app.transcript.notice(text);
        }
        Command::Export(path) => {
            let title = app.title.clone();
            let target = match path {
                Some(path) => app.cwd.join(path),
                None => super::export::default_path(&app.cwd, title.as_deref()),
            };
            let text = super::export::markdown(title.as_deref(), &app.transcript.entries);
            match std::fs::write(&target, text) {
                Ok(()) => app.transcript.notice(format!("Wrote the conversation to {}.", target.display())),
                Err(error) => app.transcript.error(format!("{}: {error}", target.display())),
            }
        }
        Command::Copy => match super::export::last_answer(&app.transcript.entries).map(str::to_owned) {
            Some(answer) => match super::export::copy(&answer) {
                Ok(()) => app.transcript.notice("Copied the last answer."),
                Err(error) => app.transcript.error(error),
            },
            None => app.transcript.notice("No answer to copy yet."),
        },
        Command::Logout => app.spawn(|sdk| match sdk.providers() {
            Ok(providers) => {
                Msg::Providers(providers.into_iter().filter(|p| p.account.is_some()).map(|p| p.provider).collect())
            }
            Err(error) => Msg::Error(error.to_string()),
        }),
        Command::ReleaseNotes => app.transcript.notice(release_notes()),
        Command::OutputStyle(named) => output_style(app, named),
        Command::TerminalSetup => app.transcript.notice(concat!(
            "New lines: Alt+Enter or Ctrl+J work everywhere. For Shift+Enter, use a terminal that reports it \
             (Ghostty, kitty, WezTerm, iTerm2 with \"Report modifiers using CSI u\"), or map Shift+Enter to \
             send \\n in your terminal's key settings. ",
            brand::name!(),
            " asks the terminal for these keys when it can."
        )),
        _ => {}
    }
}

pub fn styles() -> Vec<(String, String)> {
    let mut styles = vec![("default".to_owned(), format!("{}'s usual way: works, then reports", brand::NAME))];
    for (path, _) in content::builtin_files("styles/") {
        let Some(id) = path.strip_prefix("styles/").and_then(|name| name.strip_suffix(".md")) else { continue };
        let text = content::text(path);
        let about =
            text.lines().find_map(|line| line.strip_prefix("description:")).unwrap_or_default().trim().to_owned();
        styles.push((id.to_owned(), about));
    }
    styles
}

pub fn output_style(app: &mut App, named: Option<String>) {
    match named {
        Some(name) => app.set_output_style(name.to_lowercase()),
        None => {
            let current = app.output_style.as_deref().unwrap_or("default");
            let styles = styles();
            let cursor = styles.iter().position(|(id, _)| id == current).unwrap_or(0);
            let items = styles;
            app.picker = Some(Picker { kind: PickerKind::OutputStyle, title: "Output style".into(), items, cursor });
        }
    }
}

impl App {
    pub(super) fn set_output_style(&mut self, style: String) {
        if let Some(session) = &mut self.session {
            session.set_output_style(Some(style.clone()));
        }
        self.saver.remember("outputStyle", &style);
        self.transcript.notice(format!("Output style: {style}, from the next message."));
        self.output_style = Some(style);
    }
}

pub fn connected(app: &mut App, providers: Vec<ModelProvider>) {
    if providers.is_empty() {
        return app.transcript.notice("No provider is connected.");
    }
    let items = providers.iter().map(|p| (p.name().to_owned(), String::new())).collect();
    app.connected = providers;
    app.picker = Some(Picker { kind: PickerKind::Logout, title: "Forget which provider?".into(), items, cursor: 0 });
}

impl App {
    pub(super) fn logout_picked(&mut self, cursor: usize) {
        let Some(provider) = self.connected.get(cursor).copied() else { return };
        self.spawn(move |sdk| match sdk.disconnect_provider(provider) {
            Ok(_) => Msg::Notice(format!("Forgot {}.", provider.name())),
            Err(error) => Msg::Error(error.to_string()),
        });
    }
}

fn status(app: &App) -> String {
    let session = app.session.as_ref().and_then(sdk::Session::id).map_or("not started".to_owned(), |id| id.0);
    let model = app.model().map_or("none".to_owned(), |m| format!("{} ({})", m.label, m.id));
    let mut text = format!(
        "{brand_name} {}\n  Model    {model}\n  Mode     {}\n  Effort   {}\n  Folder   {}\n  Session  {session}",
        env!("CARGO_PKG_VERSION"),
        mode_label(app.mode()),
        app.effort().cli_value(),
        app.cwd.display(),
        brand_name = brand::NAME
    );
    for dir in &app.added_dirs {
        text.push_str(&format!("\n  Added    {}", dir.display()));
    }
    if app.transcript.cost > 0.0 {
        text.push_str(&format!("\n  Cost     ${:.4}", app.transcript.cost));
    }
    text
}

fn release_notes() -> String {
    if RELEASE_NOTES.is_empty() {
        format!("{} {}", brand::NAME, env!("CARGO_PKG_VERSION"))
    } else {
        RELEASE_NOTES.to_owned()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_release_notes_are_the_newest_changelog_section_or_the_version() {
        let notes = super::release_notes();
        let version = format!("{} {}", brand::NAME, env!("CARGO_PKG_VERSION"));
        assert!(notes == version || (notes.starts_with("## ") && !notes[3..].contains("\n## ")), "{notes}");
        assert!(!notes.contains("](") && !notes.contains("https://"), "no links: {notes}");
    }
}
