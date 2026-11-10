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
                text.push_str(&format!("\n  /{name:<9} {about}"));
            }
            text.push_str("\n  /<skill>   Run a skill or custom command\n");
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
        Command::Diff => {
            let cwd = app.cwd.clone();
            app.spawn(move |_| Msg::Changes(super::git::changes(&cwd)));
        }
        Command::Memory(query) => {
            let cwd = app.cwd.clone();
            app.spawn(move |superfik| {
                let memory = superfik.memory(cwd);
                let listed = match &query {
                    Some(query) => memory.search(query).map(|found| {
                        found.into_iter().map(|(m, score)| format!("  {score:.2}  {}", m.text)).collect::<Vec<_>>()
                    }),
                    None => memory.list().map(|all| all.into_iter().map(|m| format!("  · {}", m.text)).collect()),
                };
                match listed {
                    Ok(lines) if lines.is_empty() => Msg::Notice("Nothing remembered for this project yet.".into()),
                    Ok(lines) => Msg::Notice(format!("Memory:\n{}", lines.join("\n"))),
                    Err(error) => Msg::Error(error.to_string()),
                }
            });
        }
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
    builtin.chain(skills).filter(|(name, _)| name.starts_with(typed)).take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_offer_commands_then_skills_by_prefix() {
        let skills = [("memo".to_owned(), "Writes a memo".to_owned()), ("audit".to_owned(), "Audits".to_owned())];
        let names: Vec<String> = suggestions("me", &skills).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["memory", "memo"]);
        assert_eq!(suggestions("", &skills).len(), 8, "at most eight");
    }
}
