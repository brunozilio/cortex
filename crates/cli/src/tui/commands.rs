use domain::command::Command;

use super::app::App;

const KEYS: &str = "Keys: Enter sends · Alt+Enter or Ctrl+J new line · Esc interrupts · Esc Esc rewinds · \
                    Shift+Tab mode · Ctrl+R searches past prompts · Ctrl+G edits in $EDITOR · \
                    Ctrl+V pastes an image (or drop a file) · Ctrl+O expands output · Ctrl+L redraws · \
                    PgUp/PgDn scroll · Ctrl+C quits";

pub fn run(app: &mut App, command: Command) {
    match command {
        Command::Help(name) => super::slash::help(app, name, KEYS),
        Command::New => app.new_session(),
        Command::Model(None) => app.model_picker(),
        Command::Model(Some(wanted)) => app.set_model_named(&wanted),
        Command::Mode(None) => app.mode_picker(),
        Command::Mode(Some(mode)) => app.switch_mode(mode),
        Command::Effort(None) => app.effort_picker(),
        Command::Effort(effort) => app.set_effort_from(effort),
        Command::Compact(focus) => app.compact(focus.as_deref().unwrap_or_default()),
        Command::Rewind => app.open_rewind(),
        Command::Retry => app.retry(),
        Command::Btw(question) => super::side::ask(app, question),
        Command::AddDir(typed) => super::dirs::run(app, typed),
        Command::Context => super::context::measure(app),
        Command::Resume(query) => super::resume::open(app, query),
        Command::Fork => super::resume::fork(app),
        Command::Config => super::config::open(app),
        Command::NewAgent(what) => super::config::create(app, true, &what),
        Command::NewSkill(what) => super::config::create(app, false, &what),
        Command::StatusLine(command) => super::statusline::set(app, command),
        Command::Doctor => super::info::doctor(app),
        other @ (Command::Status
        | Command::Export(_)
        | Command::Copy
        | Command::Logout
        | Command::TerminalSetup
        | Command::OutputStyle(_)) => super::info::run(app, other),
        Command::Vim => super::toggles::vim(app),
        Command::Todos => super::listings::todos(app),
        Command::Permissions => super::dirs::show_rules(app, false),
        Command::Hooks => super::dirs::show_rules(app, true),
        Command::Diffs => app.open_diffs(),
        Command::Diff => app.show_changes(),
        Command::Skills => super::listings::skills(app),
        Command::Agents => super::listings::agents(app),
        Command::Reload => super::reload::run(app),
        Command::Mcp(change) => super::mcp_servers::run(app, change),
        Command::Cost => super::cost::show(app),
        Command::Login => app.login(),
        Command::Exit => app.quit = true,
    }
}

pub fn suggestions(typed: &str, skills: &[(String, String)]) -> Vec<(String, String)> {
    let builtin = Command::HELP
        .into_iter()
        .flat_map(|(name, about)| {
            let named = name.starts_with(typed);
            let aliases = Command::ALIASES.into_iter().filter(move |(_, of)| *of == name && !named);
            std::iter::once((name.to_owned(), about.to_owned()))
                .chain(aliases.map(move |(alias, _)| (alias.to_owned(), format!("Same as /{name}: {about}"))))
        })
        .chain(super::features::help().into_iter().map(|(name, about)| (name.to_owned(), about.to_owned())));
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
        let aliases_of_clear =
            |typed: &str| suggestions(typed, &[]).into_iter().filter(|(_, about)| about.starts_with("Same as /clear"));
        assert_eq!(aliases_of_clear("").count(), 0, "an alias hides while its command is offered");
        assert_eq!(aliases_of_clear("ne").map(|(name, _)| name).collect::<Vec<_>>(), ["new"]);
        assert_eq!(
            suggestions("res", &[]).iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
            ["reset", "resume"]
        );
        assert_eq!(
            suggestions("", &skills).len(),
            Command::HELP.len() + super::super::features::help().len() + 2,
            "every match, not just a screenful"
        );
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
