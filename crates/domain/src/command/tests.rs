use super::*;
use crate::MemoryId;

#[test]
fn commands_parse_with_their_arguments() {
    assert_eq!(Command::parse("  /CLEAR "), Some(Command::New));
    assert_eq!(Command::parse("/reset"), Some(Command::New));
    assert_eq!(Command::parse("/new"), Some(Command::New));
    assert!(Command::HELP.iter().any(|(name, _)| *name == "clear"), "/clear is listed and completed");
    assert_eq!(Command::parse("/model"), Some(Command::Model(None)));
    assert_eq!(Command::parse("/diff"), Some(Command::Diff));
    assert_eq!(Command::parse("/diffs"), Some(Command::Diffs));
    assert_eq!(Command::parse("/model claude-opus-5-5"), Some(Command::Model(Some("claude-opus-5-5".into()))));
    assert_eq!(Command::parse("/mode plan"), Some(Command::Mode(Some(PermissionMode::Plan))));
    assert_eq!(Command::parse("/effort max"), Some(Command::Effort(Some(Effort::Max))));
    assert_eq!(Command::parse("/memory"), Some(Command::Memory(MemoryCommand::List)));
    assert_eq!(
        Command::parse("/memory the renderer"),
        Some(Command::Memory(MemoryCommand::Search("the renderer".into())))
    );
    assert_eq!(Command::parse("/memory Optimize"), Some(Command::Memory(MemoryCommand::Optimize)));
    assert_eq!(Command::parse("/memory undo"), Some(Command::Memory(MemoryCommand::Undo)));
    assert_eq!(Command::parse("/memory forgotten"), Some(Command::Memory(MemoryCommand::Forgotten)));
    assert_eq!(Command::parse("/memory restore 12"), Some(Command::Memory(MemoryCommand::Restore(MemoryId(12)))));
    assert_eq!(Command::parse("/memory restore"), None, "restore needs an id");
    assert_eq!(
        Command::parse("/memory undo the font"),
        Some(Command::Memory(MemoryCommand::Search("undo the font".into()))),
        "more words are a search"
    );
    assert_eq!(Command::parse("/mcp"), Some(Command::Mcp(McpCommand::List)));
    assert_eq!(Command::parse("/reload"), Some(Command::Reload));
    assert_eq!(Command::parse("/rewind"), Some(Command::Rewind));
    assert_eq!(Command::parse("/undo"), Some(Command::Rewind));
    assert_eq!(Command::parse("/retry"), Some(Command::Retry));
    assert_eq!(Command::parse("/mcp approve jira"), Some(Command::Mcp(McpCommand::Approve("jira".into()))));
    assert_eq!(Command::parse("/mcp Deny  sonar"), Some(Command::Mcp(McpCommand::Deny("sonar".into()))));
    assert_eq!(Command::parse("/mcp approve"), None, "approve needs a name");
    assert_eq!(Command::parse("/mcp login linear"), Some(Command::Mcp(McpCommand::Login("linear".into()))));
    assert_eq!(Command::parse("/mcp logout linear"), Some(Command::Mcp(McpCommand::Logout("linear".into()))));
    assert_eq!(Command::parse("/btw which file?"), Some(Command::Btw(Some("which file?".into()))));
    assert_eq!(Command::parse("/btw"), Some(Command::Btw(None)), "answered with how to use it");
    assert_eq!(Command::parse("/add-dir ../shared"), Some(Command::AddDir(Some("../shared".into()))));
    assert_eq!(Command::parse("/add-dir"), Some(Command::AddDir(None)));
    assert_eq!(Command::parse("/permissions"), Some(Command::Permissions));
    assert_eq!(Command::parse("/todos"), Some(Command::Todos));
    assert_eq!(Command::parse("/compact"), Some(Command::Compact(None)));
    assert_eq!(Command::parse("/agents"), Some(Command::Agents));
    assert_eq!(Command::parse("/agents new reviews SQL"), Some(Command::NewAgent("reviews SQL".into())));
    assert_eq!(Command::parse("/skills new deploys to fly"), Some(Command::NewSkill("deploys to fly".into())));
    assert_eq!(Command::parse("/skills new"), None);
    assert_eq!(Command::parse("/memory edit"), Some(Command::Memory(MemoryCommand::Edit)));
    assert_eq!(Command::parse("/config"), Some(Command::Config));
    assert_eq!(Command::parse("/compact the API work"), Some(Command::Compact(Some("the API work".into()))));
    assert_eq!(Command::parse("/bashes kill"), None);
}

#[test]
fn verification_shows_sets_or_clears_the_criteria() {
    assert_eq!(Command::parse("/verification"), Some(Command::Verification(VerificationCommand::Show)));
    assert_eq!(Command::parse("/verification OFF"), Some(Command::Verification(VerificationCommand::Clear)));
    assert_eq!(
        Command::parse("/verification  cargo test passes "),
        Some(Command::Verification(VerificationCommand::Set("cargo test passes".into())))
    );
    assert_eq!(Command::parse("/verify"), None, "the verify skill keeps its name");
}

#[test]
fn every_command_is_in_exactly_one_help_group() {
    for (name, _) in Command::HELP {
        let homes = Command::GROUPS.iter().filter(|(_, names)| names.contains(&name)).count();
        assert_eq!(homes, 1, "/{name}");
    }
    let grouped: usize = Command::GROUPS.iter().map(|(_, names)| names.len()).sum();
    assert_eq!(grouped, Command::HELP.len(), "no group names a command that does not exist");
}

#[test]
fn resume_may_search_the_titles() {
    assert_eq!(Command::parse("/resume"), Some(Command::Resume(None)));
    assert_eq!(Command::parse("/continue login bug"), Some(Command::Resume(Some("login bug".into()))));
}

#[test]
fn help_names_one_command_or_lists_them_all() {
    assert_eq!(Command::parse("/help"), Some(Command::Help(None)));
    assert_eq!(Command::parse("/? mode"), Some(Command::Help(Some("mode".into()))));
}

#[test]
fn anything_else_is_left_to_the_engine() {
    assert_eq!(Command::parse("/new explain this"), None, "a bare command takes no argument");
    assert_eq!(Command::parse("/mode sideways"), None);
    assert_eq!(Command::parse("/audit src"), None, "a custom command");
    assert_eq!(Command::parse("/etc/hosts"), None, "a path");
    assert_eq!(Command::parse("hello"), None);
}

#[test]
fn modes_and_levels_read_back_from_their_names() {
    for mode in PermissionMode::ALL {
        assert_eq!(mode_named(mode_label(mode)), Some(mode));
    }
    for level in Effort::ALL {
        assert_eq!(level_named(level.cli_value()), Some(level));
    }
    let mut mode = PermissionMode::Ask;
    for expected in [PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::Bypass, PermissionMode::Ask] {
        mode = mode.next();
        assert_eq!(mode, expected);
    }
}

#[test]
fn help_names_only_commands_that_parse() {
    for (name, _) in Command::HELP {
        assert!(Command::parse(&format!("/{name}")).is_some(), "{name}");
    }
}

#[test]
fn every_alias_runs_the_command_it_names() {
    for (alias, name) in Command::ALIASES {
        assert!(Command::HELP.iter().any(|(listed, _)| *listed == name), "/{name}");
        assert_eq!(Command::parse(&format!("/{alias}")), Command::parse(&format!("/{name}")), "/{alias}");
    }
}
