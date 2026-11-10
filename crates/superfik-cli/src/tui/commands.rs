//! The chat's own `/commands`.

use superfik_core::command::{Command, McpCommand};
use superfik_sdk::PermissionMode;

use superfik_core::command::mode_label;

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};

const KEYS: &str = "Keys: Enter sends · Alt+Enter or Ctrl+J new line · Esc interrupts · Shift+Tab mode · \
                    Ctrl+V pastes an image (or drop a file) · Ctrl+O expands output · PgUp/PgDn scroll · \
                    Ctrl+C quits";

pub fn run(app: &mut App, command: Command) {
    match command {
        Command::Help => {
            let mut text = String::from("Commands:");
            for (name, about) in Command::HELP {
                text.push_str(&format!("\n  /{name:<12} {about}"));
            }
            text.push_str("\n  /<skill>      Run a skill or custom command\n");
            text.push_str(KEYS);
            app.transcript.notice(text);
        }
        Command::New => app.new_session(),
        Command::Model(None) => {
            if app.models.is_empty() {
                return app.transcript.notice("No models yet. Run `superfik login` to connect a provider.");
            }
            let current = app.model().map(|m| m.id.clone());
            let items = app.models.iter().map(|m| (m.label.clone(), m.id.clone())).collect();
            let cursor = app.models.iter().position(|m| Some(&m.id) == current.as_ref()).unwrap_or(0);
            app.picker = Some(Picker { kind: PickerKind::Model, title: "Model".into(), items, cursor });
        }
        Command::Model(Some(wanted)) => match app.models.iter().find(|m| m.is_named(&wanted)).cloned() {
            Some(model) => app.set_model(model),
            None => app.transcript.error(format!("No model named {wanted}. /model lists them.")),
        },
        Command::Mode(None) => {
            let modes = [
                (PermissionMode::Ask, "Ask before edits and commands"),
                (PermissionMode::AcceptEdits, "Edit files freely, ask before commands"),
                (PermissionMode::Plan, "Read and plan only"),
                (PermissionMode::Bypass, "Never ask (a safety check still grades risky actions)"),
            ];
            let cursor = modes.iter().position(|(mode, _)| *mode == app.mode()).unwrap_or(0);
            let items = modes.iter().map(|(mode, about)| (mode_label(*mode).to_owned(), (*about).to_owned())).collect();
            app.picker = Some(Picker { kind: PickerKind::Mode, title: "Permission mode".into(), items, cursor });
        }
        Command::Mode(Some(mode)) => {
            app.set_mode(mode);
            app.transcript.notice(format!("Mode: {}", mode_label(mode)));
        }
        Command::Effort(effort) => app.set_effort_from(effort),
        Command::Compact => app.compact(),
        Command::Rewind => app.open_rewind(),
        Command::Retry => app.retry(),
        Command::Btw(question) => super::side::ask(app, question),
        Command::Verification(change) => super::verification::run(app, change),
        Command::Diffs => {
            app.diffs = Some(super::diff_view::DiffView::new(super::git::Scope::Uncommitted));
            app.load_diffs(super::git::Scope::Uncommitted);
        }
        Command::Diff => {
            let cwd = app.cwd.clone();
            app.spawn(move |_| Msg::Changes(super::git::changes(&cwd)));
        }
        Command::Memory(command) => super::memory::run(app, command),
        Command::Skills => {
            let cwd = app.cwd.clone();
            app.spawn(move |superfik| match superfik.skills(&cwd) {
                Ok(skills) if skills.is_empty() => Msg::Notice("No skills or commands found.".into()),
                Ok(skills) => {
                    let lines: Vec<String> =
                        skills.iter().map(|s| format!("  /{:<20} {}", s.name, s.description)).collect();
                    Msg::Notice(format!("Skills and commands:\n{}", lines.join("\n")))
                }
                Err(error) => Msg::Error(error.to_string()),
            });
        }
        Command::Agents => {
            let cwd = app.cwd.clone();
            app.spawn(move |superfik| match superfik.agents(&cwd) {
                Ok(agents) => {
                    let lines: Vec<String> =
                        agents.iter().map(|a| format!("  {:<20} {}", a.name, a.description)).collect();
                    Msg::Notice(format!("Agents:\n{}", lines.join("\n")))
                }
                Err(error) => Msg::Error(error.to_string()),
            });
        }
        Command::Reload => {
            app.load(false);
            let cwd = app.cwd.clone();
            app.spawn(move |superfik| {
                let found = (|| -> Result<_, superfik_sdk::Error> {
                    Ok((superfik.skills(&cwd)?, superfik.agents(&cwd)?, superfik.mcp_servers(&cwd)?))
                })();
                match found {
                    Ok((skills, agents, servers)) => {
                        let names: Vec<String> = skills.iter().map(|s| s.name.clone()).collect();
                        let summary = super::reload::summary(&names, &agents, &servers);
                        Msg::Reloaded { skills: skills.into_iter().map(|s| (s.name, s.description)).collect(), summary }
                    }
                    Err(error) => Msg::Error(error.to_string()),
                }
            });
        }
        Command::Mcp(change) => {
            let cwd = app.cwd.clone();
            app.spawn(move |superfik| {
                let servers = match &change {
                    McpCommand::List => superfik.mcp_servers(&cwd),
                    McpCommand::Approve(name) => superfik.approve_mcp_server(&cwd, name, true),
                    McpCommand::Deny(name) => superfik.approve_mcp_server(&cwd, name, false),
                };
                match servers {
                    Ok(servers) => {
                        let shown = crate::mcp::render(&servers).replace("superfik mcp approve", "/mcp approve");
                        let note =
                            if change == McpCommand::List { "" } else { "\nThe change applies from the next message." };
                        Msg::Notice(format!("MCP servers:\n{}{note}", shown.trim_end()))
                    }
                    Err(error) => Msg::Error(error.to_string()),
                }
            });
        }
        Command::Cost => {
            let cost = app.transcript.cost;
            app.transcript.notice(format!("This session cost ${cost:.4} (when the provider prices it)."));
        }
        Command::Login => app.login(),
        Command::Exit => app.quit = true,
    }
}

/// Built-in commands and skills whose name starts with `typed`.
pub fn suggestions(typed: &str, skills: &[(String, String)]) -> Vec<(String, String)> {
    let builtin = Command::HELP.iter().map(|(name, about)| ((*name).to_owned(), (*about).to_owned()));
    let skills = skills.iter().map(|(name, about)| (name.clone(), about.clone()));
    builtin.chain(skills).filter(|(name, _)| name.starts_with(typed)).collect()
}

/// How many `/` suggestions show at once; the arrows scroll the rest.
pub const SHOWN: usize = 8;

/// The suggestion after (or before) `current` among `len`, wrapping round.
pub fn step(current: usize, len: usize, forward: bool) -> usize {
    match len {
        0 => 0,
        _ if forward => (current % len + 1) % len,
        _ => (current % len + len - 1) % len,
    }
}

/// Which of `len` suggestions show, scrolled so `chosen` is on screen.
pub fn shown(len: usize, chosen: usize) -> std::ops::Range<usize> {
    let start = (chosen + 1).saturating_sub(SHOWN);
    start..(start + SHOWN).min(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_offer_commands_then_skills_by_prefix() {
        let skills = [("memo".to_owned(), "Writes a memo".to_owned()), ("audit".to_owned(), "Audits".to_owned())];
        let names: Vec<String> = suggestions("me", &skills).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["memory", "memo"]);
        assert_eq!(suggestions("", &skills).len(), Command::HELP.len() + 2, "every match, not just a screenful");
    }

    #[test]
    fn the_arrows_wrap_around_the_suggestions() {
        assert_eq!(step(0, 20, false), 19, "up from the first goes to the last");
        assert_eq!(step(19, 20, true), 0, "down from the last goes to the first");
        assert_eq!(step(4, 20, true), 5);
        assert_eq!(step(0, 0, true), 0, "nothing to move through");
    }

    #[test]
    fn the_shown_suggestions_follow_the_chosen_one() {
        assert_eq!(shown(5, 2), 0..5, "a short list shows whole");
        assert_eq!(shown(20, 3), 0..SHOWN, "the top while the choice is on screen");
        assert_eq!(shown(20, 12), 12 + 1 - SHOWN..13, "scrolled so the choice is the last row");
        assert_eq!(shown(20, 19), 20 - SHOWN..20);
    }
}
