//! The terminal chat's own `/commands`: what each is called and how typed
//! text names one. Any other `/name` is a skill or custom command the
//! engine expands. Pure.

use crate::{Effort, PermissionMode};

mod looping;
mod sub;
pub use looping::loop_prompt;
pub use sub::{McpCommand, MemoryCommand, VerificationCommand};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    New,
    Model(Option<String>),
    Mode(Option<PermissionMode>),
    Effort(Option<Effort>),
    /// Summarize the conversation, keeping what the words after it name.
    Compact(Option<String>),
    /// Back to an earlier prompt: its files and the conversation as they were.
    Rewind,
    /// The last prompt again, from its checkpoint.
    Retry,
    Diff,
    /// The changes full screen, file by file.
    Diffs,
    Memory(MemoryCommand),
    /// A side question, answered without tools and kept out of the conversation.
    Btw(Option<String>),
    /// Lets the agent read a folder like the project's own; bare, lists
    /// the folders added.
    AddDir(Option<String>),
    /// The permission rules and added folders the agent follows here.
    Permissions,
    /// The agent's task list.
    Todos,
    /// What fills the context window.
    Context,
    /// The commands left in the background; `/bashes kill <id>` stops one.
    Bashes(Option<String>),
    /// The prompts the agent scheduled; `/schedules delete <id>` removes one.
    Schedules(Option<String>),
    /// A task now and again later: `/loop [interval] <task>`.
    Loop(String),
    /// Vim keys in the prompt, on or off.
    Vim,
    /// The mouse wheel scrolls the conversation, on or off.
    Mouse,
    /// Go back to an earlier conversation of this folder.
    Resume,
    /// The next message goes on in a copy of this conversation.
    Fork,
    /// Write the conversation as Markdown (to a path, or one named after it).
    Export(Option<String>),
    /// Put the agent's last answer on the clipboard.
    Copy,
    /// The model, mode, folders, session and version in use.
    Status,
    /// Forget a provider's credentials.
    Logout,
    /// What changed in this version.
    ReleaseNotes,
    /// How to make Shift+Enter add a line in this terminal.
    TerminalSetup,
    /// How the agent talks: a picker, or `/output-style <name>`.
    OutputStyle(Option<String>),
    /// The hooks a run here fires.
    Hooks,
    /// The status line drawn by the user's command, or `off`.
    StatusLine(Option<String>),
    /// Whether this installation can work in the folder.
    Doctor,
    /// Where to report a problem or an idea.
    Feedback,
    Skills,
    Agents,
    /// `/agents new <what it does>`: the agent writes a sub-agent for it.
    NewAgent(String),
    /// `/skills new <what it does>`: the agent writes a skill for it.
    NewSkill(String),
    /// The chat's settings, to change one.
    Config,
    Reload,
    Mcp(McpCommand),
    /// What the run's work is judged against before it completes.
    Verification(VerificationCommand),
    Cost,
    Login,
    Exit,
}

impl Command {
    /// Every command with what it does, for `/help` and completion.
    pub const HELP: [(&'static str, &'static str); 43] = [
        ("help", "Show the commands and keys"),
        ("new", "Start a new conversation (also /clear)"),
        ("resume", "Go back to an earlier conversation of this folder"),
        ("fork", "Go on in a copy of this conversation, leaving it as it is"),
        ("model", "Pick the model, or /model <id>"),
        ("mode", "Pick ask, edits, plan or bypass"),
        ("effort", "Set low, medium, high, xhigh or max"),
        ("output-style", "How the agent talks: default, concise, proactive, explanatory, learning"),
        ("compact", "Summarize the conversation to free context, /compact <what to keep>"),
        ("rewind", "Go back to an earlier prompt: its files and the conversation"),
        ("retry", "Send the last prompt again, from where it started"),
        ("diff", "Show the project's uncommitted changes"),
        ("diffs", "Browse the changes full screen: uncommitted, staged or branch"),
        ("memory", "List the project's memory, /memory <search>|optimize|undo|forgotten|restore <id>|edit"),
        ("btw", "Ask a side question without interrupting: /btw <question>"),
        ("add-dir", "Let the agent read a folder like the project: /add-dir <path>"),
        ("permissions", "Show the allow, ask and deny rules the agent follows here"),
        ("hooks", "Show the hooks a run here fires"),
        ("statusline", "Draw the status line with your command, or /statusline off"),
        ("todos", "Show the agent's task list (Ctrl+T shows or hides it)"),
        ("context", "Show what fills the context window and what is left"),
        ("bashes", "List commands left in the background, /bashes kill <id> stops one"),
        ("schedules", "List scheduled prompts, /schedules delete <id> removes one"),
        ("loop", "Run a task now and again later: /loop [5m|2h|1d] <task>"),
        ("vim", "Turn vim keys in the prompt on or off"),
        ("mouse", "Let the mouse wheel scroll (select text with Shift/Option)"),
        ("skills", "List skills and commands, /skills new <what it does> writes one"),
        ("agents", "List agents, /agents new <what it does> writes one"),
        ("config", "Change the chat's settings"),
        ("reload", concat!("Reread ~/", brand::dir!(), " and the project: plugins, skills, agents, MCP")),
        ("mcp", "List MCP servers, or /mcp approve|deny|login|logout <name>"),
        ("verification", "Judge runs: /verification <criteria>, or off"),
        ("cost", "Show this session's cost (also /usage)"),
        ("status", "Show the model, mode, folders, session and version"),
        ("export", "Write the conversation as Markdown, /export <path>"),
        ("copy", "Copy the agent's last answer"),
        ("logout", "Forget a provider's credentials"),
        ("release-notes", "What changed in this version"),
        ("doctor", "Check that this installation can work here"),
        ("feedback", "Where to report a problem or an idea"),
        ("terminal-setup", "How to make Shift+Enter add a line in this terminal"),
        ("login", "Connect a provider (API key or subscription)"),
        ("exit", "Quit (also /quit)"),
    ];

    /// `None` when `text` is not one of these (a skill, a command, or a
    /// path typed as a prompt).
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim().strip_prefix('/')?;
        let (name, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let rest = rest.trim();
        let argument = (!rest.is_empty()).then(|| rest.to_owned());
        let bare = |command: Self| argument.is_none().then_some(command);
        match name.to_ascii_lowercase().as_str() {
            "help" | "?" => bare(Self::Help),
            "new" | "clear" => bare(Self::New),
            "model" => Some(Self::Model(argument)),
            "mode" => match &argument {
                None => Some(Self::Mode(None)),
                Some(mode) => mode_named(mode).map(|mode| Self::Mode(Some(mode))),
            },
            "effort" => match &argument {
                None => Some(Self::Effort(None)),
                Some(level) => level_named(level).map(|level| Self::Effort(Some(level))),
            },
            "compact" => Some(Self::Compact(argument)),
            "rewind" | "undo" => bare(Self::Rewind),
            "retry" => bare(Self::Retry),
            "diff" => bare(Self::Diff),
            "diffs" => bare(Self::Diffs),
            "memory" => MemoryCommand::parse(argument).map(Self::Memory),
            "btw" => Some(Self::Btw(argument)),
            "add-dir" => Some(Self::AddDir(argument)),
            "permissions" | "allowed-tools" => bare(Self::Permissions),
            "todos" | "tasks" => bare(Self::Todos),
            "context" => bare(Self::Context),
            "vim" => bare(Self::Vim),
            "mouse" => bare(Self::Mouse),
            "resume" | "continue" => bare(Self::Resume),
            "fork" | "branch" => bare(Self::Fork),
            "schedules" => match argument.as_deref().map(|a| a.split_once(' ').unwrap_or((a, ""))) {
                None => Some(Self::Schedules(None)),
                Some(("delete", id)) if !id.trim().is_empty() => Some(Self::Schedules(Some(id.trim().to_owned()))),
                Some(_) => None,
            },
            "loop" => Some(Self::Loop(argument.unwrap_or_default())),
            "bashes" => match argument.as_deref().map(|a| a.split_once(' ').unwrap_or((a, ""))) {
                None => Some(Self::Bashes(None)),
                Some(("kill", id)) if !id.trim().is_empty() => Some(Self::Bashes(Some(id.trim().to_owned()))),
                Some(_) => None,
            },
            "skills" | "commands" | "agents" => match argument.as_deref().map(|a| a.split_once(' ').unwrap_or((a, "")))
            {
                None if name.eq_ignore_ascii_case("agents") => Some(Self::Agents),
                None => Some(Self::Skills),
                Some(("new", what)) if !what.trim().is_empty() && name.eq_ignore_ascii_case("agents") => {
                    Some(Self::NewAgent(what.trim().to_owned()))
                }
                Some(("new", what)) if !what.trim().is_empty() => Some(Self::NewSkill(what.trim().to_owned())),
                Some(_) => None,
            },
            "config" | "settings" => bare(Self::Config),
            "reload" => bare(Self::Reload),
            "mcp" => McpCommand::parse(argument.as_deref()).map(Self::Mcp),
            "verification" => Some(Self::Verification(VerificationCommand::parse(argument))),
            "cost" | "usage" => bare(Self::Cost),
            "status" => bare(Self::Status),
            "export" => Some(Self::Export(argument)),
            "copy" => bare(Self::Copy),
            "logout" => bare(Self::Logout),
            "release-notes" => bare(Self::ReleaseNotes),
            "terminal-setup" => bare(Self::TerminalSetup),
            "output-style" => Some(Self::OutputStyle(argument)),
            "hooks" => bare(Self::Hooks),
            "statusline" => Some(Self::StatusLine(argument)),
            "doctor" => bare(Self::Doctor),
            "feedback" | "bug" => bare(Self::Feedback),
            "login" => bare(Self::Login),
            "exit" | "quit" => bare(Self::Exit),
            _ => None,
        }
    }
}

/// The short name a mode is typed and shown as (`/mode edits`).
pub const fn mode_label(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Ask => "ask",
        PermissionMode::AcceptEdits => "edits",
        PermissionMode::Plan => "plan",
        PermissionMode::Bypass => "bypass",
    }
}

pub fn mode_named(name: &str) -> Option<PermissionMode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "ask" | "default" => PermissionMode::Ask,
        "edits" | "acceptedits" | "accept-edits" => PermissionMode::AcceptEdits,
        "plan" => PermissionMode::Plan,
        "bypass" | "yolo" => PermissionMode::Bypass,
        _ => return None,
    })
}

pub fn level_named(name: &str) -> Option<Effort> {
    Some(match name.to_ascii_lowercase().as_str() {
        "low" => Effort::Low,
        "medium" => Effort::Medium,
        "high" => Effort::High,
        "xhigh" => Effort::XHigh,
        "max" => Effort::Max,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
