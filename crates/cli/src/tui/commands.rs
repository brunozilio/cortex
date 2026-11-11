use domain::command::{Command, McpCommand};
use sdk::PermissionMode;

use domain::command::mode_label;

use super::app::{App, Msg};
use super::choices::{Picker, PickerKind};

const KEYS: &str = "Keys: Enter sends · Alt+Enter or Ctrl+J new line · Esc interrupts · Esc Esc rewinds · \
                    Shift+Tab mode · Ctrl+R searches past prompts · Ctrl+G edits in $EDITOR · \
                    Ctrl+V pastes an image (or drop a file) · Ctrl+O expands output · Ctrl+L redraws · \
                    PgUp/PgDn scroll · Ctrl+C quits";

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
                return app.transcript.notice(concat!(
                    "No models yet. Run `",
                    brand::command!(),
                    " login` to connect a provider."
                ));
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
        Command::Compact(focus) => app.compact(focus.as_deref().unwrap_or_default()),
        Command::Rewind => app.open_rewind(),
        Command::Retry => app.retry(),
        Command::Btw(question) => super::side::ask(app, question),
        Command::AddDir(typed) => super::dirs::run(app, typed),
        Command::Context => super::context::measure(app),
        Command::Resume => super::resume::open(app),
        Command::Fork => match app.session.as_ref().filter(|session| session.id().is_some()) {
            Some(session) => {
                session.fork_next();
                app.transcript
                    .notice("The next message goes on in a copy of this conversation; this one stays as it is.");
            }
            None => app.transcript.notice("Nothing to fork yet: send a message first."),
        },
        Command::Config => super::config::open(app),
        Command::NewAgent(what) => super::config::create(app, true, &what),
        Command::NewSkill(what) => super::config::create(app, false, &what),
        Command::StatusLine(command) => super::statusline::set(app, command),
        Command::Doctor => {
            let cwd = app.cwd.clone();
            app.spawn(move |sdk| Msg::Notice(crate::doctor::render(&crate::doctor::checks(sdk, &cwd))));
        }
        Command::Feedback => app.transcript.notice(concat!(
            "Report a problem or an idea at https://github.com/",
            brand::repository!(),
            "/issues (/status and /doctor show what to include)."
        )),
        other @ (Command::Status
        | Command::Export(_)
        | Command::Copy
        | Command::Logout
        | Command::ReleaseNotes
        | Command::TerminalSetup
        | Command::OutputStyle(_)) => super::info::run(app, other),
        Command::Mouse => {
            app.mouse = !app.mouse;
            app.saver.remember("mouse", if app.mouse { "on" } else { "off" });
            app.transcript.notice(if app.mouse {
                "The wheel scrolls now. Select text with Shift held (Option on macOS)."
            } else {
                "The wheel is the terminal's again."
            });
        }
        Command::Vim => {
            app.vim = if app.vim.is_some() { None } else { Some(super::vim::Vim::default()) };
            app.saver.remember("vim", if app.vim.is_some() { "on" } else { "off" });
            app.transcript.notice(if app.vim.is_some() {
                "Vim keys on: Esc for normal mode, i to insert."
            } else {
                "Vim keys off."
            });
        }
        Command::Schedules(delete) => super::schedules::list(app, delete),
        Command::Loop(text) => super::schedules::looped(app, &text),
        Command::Bashes(kill) => app.spawn(move |sdk| match sdk.shells(kill.as_deref()) {
            Ok(shells) if shells.is_empty() => Msg::Notice("No commands in the background.".into()),
            Ok(shells) => {
                let lines: Vec<String> = shells
                    .iter()
                    .map(|(id, command, running)| {
                        format!("  {id:<10} {} {command}", if *running { "running " } else { "finished" })
                    })
                    .collect();
                Msg::Notice(format!("In the background (/bashes kill <id> stops one):\n{}", lines.join("\n")))
            }
            Err(error) => Msg::Error(error.to_string()),
        }),
        Command::Todos => {
            let tasks = &app.transcript.tasks;
            let text = if tasks.is_empty() {
                "No tasks yet: the agent makes a list for work with several steps.".to_owned()
            } else {
                let lines: Vec<String> = tasks
                    .iter()
                    .map(|task| {
                        let mark = match task.status.as_str() {
                            "completed" => "☒",
                            "in_progress" => "◼",
                            _ => "☐",
                        };
                        format!("  {mark} {}", task.subject)
                    })
                    .collect();
                format!("Tasks:\n{}", lines.join("\n"))
            };
            app.transcript.notice(text);
        }
        Command::Permissions | Command::Hooks => {
            let (cwd, hooks) = (app.cwd.clone(), command == Command::Hooks);
            app.spawn(move |sdk| match sdk.permissions(&cwd) {
                Ok(info) if hooks => Msg::Notice(super::dirs::hooks(&info.hooks)),
                Ok(info) => Msg::Notice(super::dirs::permissions(&info)),
                Err(error) => Msg::Error(error.to_string()),
            });
        }
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
            app.spawn(move |sdk| match sdk.skills(&cwd) {
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
            app.spawn(move |sdk| match sdk.agents(&cwd) {
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
            app.spawn(move |sdk| {
                let found = (|| -> Result<_, sdk::Error> {
                    Ok((sdk.skills(&cwd)?, sdk.agents(&cwd)?, sdk.mcp_servers(&cwd)?))
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
            if let McpCommand::Login(name) = &change {
                app.transcript.notice(format!("Signing in to {name} in the browser…"));
            }
            app.spawn(move |sdk| {
                let servers = match &change {
                    McpCommand::List => sdk.mcp_servers(&cwd),
                    McpCommand::Approve(name) => sdk.approve_mcp_server(&cwd, name, true),
                    McpCommand::Deny(name) => sdk.approve_mcp_server(&cwd, name, false),
                    McpCommand::Login(name) => sdk.sign_in_mcp_server(&cwd, name, crate::login::open),
                    McpCommand::Logout(name) => sdk.sign_out_mcp_server(&cwd, name),
                };
                match servers {
                    Ok(servers) => {
                        let shown = crate::mcp::render(&servers)
                            .replace(concat!(brand::command!(), " mcp approve"), "/mcp approve");
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

pub fn suggestions(typed: &str, skills: &[(String, String)]) -> Vec<(String, String)> {
    let builtin = Command::HELP.iter().map(|(name, about)| ((*name).to_owned(), (*about).to_owned()));
    let skills = skills.iter().map(|(name, about)| (name.clone(), about.clone()));
    builtin.chain(skills).filter(|(name, _)| name.starts_with(typed)).collect()
}

pub const SHOWN: usize = 8;

pub fn step(current: usize, len: usize, forward: bool) -> usize {
    match len {
        0 => 0,
        _ if forward => (current % len + 1) % len,
        _ => (current % len + len - 1) % len,
    }
}

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
