//! Permission rules from settings (`permissions.allow/ask/deny`): which tool
//! calls run without asking, which always ask and which are refused, whatever
//! the permission mode.
//!
//! A rule is a tool name, optionally with a specifier: `Bash`,
//! `Bash(git diff:*)` (a command prefix), `Bash(npm run *)` (a pattern),
//! `Edit(src/**)` or `Read(~/notes/*.md)` (gitignore-like paths: from the
//! project, `~/` from home, `//` from the root; without a slash at any
//! depth below the project), `mcp__server` or `mcp__server__tool`. Deny wins
//! over ask, ask over allow. Pure.
//!
//! Matching errs toward asking: deny and ask rules see a command as the
//! shell runs it (wrappers, assignments and paths stripped) and paths in
//! any case; allow rules see the command as written and paths exactly, and
//! never cover a command hiding another (`$(…)`, `bash -c`…) or writing a
//! file (`>`). A hidden command asks whenever a deny or ask rule could
//! have caught it.

mod pattern;
mod site;

pub use site::{is_private, site};

#[cfg(test)]
mod tests;

/// What a matching rule says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Allow,
    Ask,
    Deny,
}

/// What `/permissions` shows: the rules a run in a folder follows, as
/// written, and the folders it reads like the project.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsInfo {
    pub rules: Vec<(RuleKind, String)>,
    pub added_dirs: Vec<String>,
    /// The hooks a run there fires (`/hooks`).
    #[serde(default)]
    pub hooks: Vec<HookInfo>,
}

/// One hook as `/hooks` shows it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HookInfo {
    pub event: String,
    /// Which tools (or triggers) it is for; empty for all.
    pub matcher: String,
    /// What it runs: a command, a URL, or the start of its prompt.
    pub runs: String,
}

/// What a tool call acts on, for its rules to match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    /// A shell command.
    Command(&'a str),
    /// An absolute file path.
    Path(&'a str),
    /// An absolute folder searched with everything inside it (`Glob`,
    /// `Grep`).
    Tree(&'a str),
    /// Anything else (matched exactly by a specifier).
    Other(&'a str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    text: String,
    tool: String,
    specifier: Option<String>,
}

impl Rule {
    fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (tool, specifier) = match text.split_once('(') {
            None => (text, None),
            Some((tool, rest)) => {
                let specifier = rest.strip_suffix(')').ok_or_else(|| format!("rule {text:?} misses its `)`"))?;
                if specifier.trim().is_empty() {
                    return Err(format!("rule {text:?} has an empty specifier; write just the tool name"));
                }
                (tool.trim(), Some(specifier.trim().to_owned()))
            }
        };
        if tool.is_empty() || !tool.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err(format!("rule {text:?} does not start with a tool name"));
        }
        Ok(Self { text: text.to_owned(), tool: tool.to_owned(), specifier })
    }

    /// Whether the rule names this tool: the tool itself, its family
    /// (`Edit` for the editing tools, `Read` for the reading ones) or, for
    /// `mcp__server`, a tool of exactly that server.
    fn names(&self, tool: &str) -> bool {
        if self.tool == tool {
            return true;
        }
        match self.tool.as_str() {
            "Edit" => matches!(tool, "Write" | "NotebookEdit"),
            "Read" => matches!(tool, "Glob" | "Grep" | "LSP"),
            server if server.starts_with("mcp__") && server.matches("__").count() == 1 => tool
                .strip_prefix(server)
                .and_then(|rest| rest.strip_prefix("__"))
                .is_some_and(|name| !name.is_empty() && !name.contains("__")),
            _ => false,
        }
    }
}

/// The allow rule "don't ask again" offers for `tool` on `subject`: a
/// command's program and subcommand (`Bash(cargo test:*)`), or the folder
/// of a file read outside the project (`Read(//home/ana/notes/**)`, a
/// searched folder itself). `None` when no rule fits safely: several or
/// hidden commands, a program by path, assignments, edits.
pub fn suggest(tool: &str, subject: &str, outside: bool) -> Option<String> {
    if tool == "WebFetch" {
        return site(subject).map(|host| format!("WebFetch(domain:{host})"));
    }
    if tool == "Bash" {
        let (parts, balanced) = pattern::split_command(subject);
        let [command] = parts.as_slice() else { return None };
        if !balanced || pattern::hides_more(command) {
            return None;
        }
        let collapsed = pattern::collapsed(command);
        let mut words = collapsed.split(' ');
        let program = words.next().filter(|w| w.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)))?;
        if program.starts_with('.') {
            return None;
        }
        let Some(next) = words.next() else { return Some(format!("Bash({program})")) };
        let subcommand = next.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)) && !next.starts_with('-');
        return Some(if subcommand { format!("Bash({program} {next}:*)") } else { format!("Bash({program}:*)") });
    }
    if outside && matches!(tool, "Read" | "Glob" | "Grep" | "LSP") {
        let path = subject.trim_end_matches('/');
        let folder = if tool == "Read" || tool == "LSP" { path.rsplit_once('/').map(|(dir, _)| dir)? } else { path };
        return Some(format!("Read(//{}/**)", folder.trim_start_matches('/')));
    }
    None
}

/// The rules of a list as typed on a command line (`Bash(git diff:*) Edit,
/// Read`): split at commas and spaces outside parentheses.
pub fn split_list(text: &str) -> Vec<String> {
    let (mut rules, mut current, mut depth) = (Vec::new(), String::new(), 0usize);
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' | ' ' | '\t' if depth == 0 => {
                if !current.is_empty() {
                    rules.push(std::mem::take(&mut current));
                }
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    if !current.is_empty() {
        rules.push(current);
    }
    rules
}

/// One command of a line, as written and as the shell runs it.
struct Part<'a> {
    target: Target<'a>,
    written: String,
    run: String,
    hides: bool,
}

/// The rules of every settings file, merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionRules {
    rules: Vec<(RuleKind, Rule)>,
}

impl PermissionRules {
    /// Adds rule `text`, or says why it is not one.
    pub fn add(&mut self, kind: RuleKind, text: &str) -> Result<(), String> {
        self.rules.push((kind, Rule::parse(text)?));
        Ok(())
    }

    /// [`add`](Self::add) for rules known to be valid.
    pub fn with(mut self, kind: RuleKind, text: &str) -> Self {
        if let Ok(rule) = Rule::parse(text) {
            self.rules.push((kind, rule));
        }
        self
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Every rule as written in settings, deny first, then ask, then allow.
    pub fn listed(&self) -> Vec<(RuleKind, String)> {
        let mut listed: Vec<_> = self.rules.iter().map(|(kind, rule)| (*kind, rule.text.clone())).collect();
        listed.sort_by_key(|(kind, _)| match kind {
            RuleKind::Deny => 0,
            RuleKind::Ask => 1,
            RuleKind::Allow => 2,
        });
        listed
    }

    /// What the rules say about `tool` acting on `target`, if any names it.
    /// A command is split at `&&`, `||`, `;`, `|` and new lines: one part
    /// denied or asked about is enough, and it is allowed only when every
    /// part is.
    pub fn decide(&self, tool: &str, target: &Target<'_>, cwd: &str, home: Option<&str>) -> Option<RuleKind> {
        self.ruling(tool, target, cwd, home).map(|(kind, _)| kind)
    }

    /// [`decide`](Self::decide), with the deny or ask rule that decided (as
    /// written in settings) to tell the agent; empty for an allow.
    pub fn ruling(&self, tool: &str, target: &Target<'_>, cwd: &str, home: Option<&str>) -> Option<(RuleKind, String)> {
        let parts = parts(target);
        let named = |kind: RuleKind| {
            self.rules.iter().filter(move |(rule_kind, rule)| *rule_kind == kind && rule.names(tool)).map(|(_, r)| r)
        };
        for kind in [RuleKind::Deny, RuleKind::Ask] {
            let found = parts.iter().find_map(|part| named(kind).find(|rule| restricts(rule, part, cwd, home)));
            if let Some(rule) = found {
                return Some((kind, rule.text.clone()));
            }
        }
        if parts.iter().any(|part| part.hides) {
            let watching = named(RuleKind::Deny).chain(named(RuleKind::Ask)).find(|rule| rule.specifier.is_some());
            if let Some(rule) = watching {
                return Some((RuleKind::Ask, rule.text.clone()));
            }
        }
        let allowed = |part: &Part<'_>| named(RuleKind::Allow).any(|rule| allows(rule, part, cwd, home));
        (!parts.is_empty() && parts.iter().all(allowed)).then(|| (RuleKind::Allow, String::new()))
    }
}

fn parts<'a>(target: &Target<'a>) -> Vec<Part<'a>> {
    let Target::Command(line) = target else {
        return vec![Part { target: *target, written: String::new(), run: String::new(), hides: false }];
    };
    let (commands, balanced) = pattern::split_command(line);
    commands
        .into_iter()
        .map(|command| Part {
            target: Target::Command(command),
            written: pattern::collapsed(command),
            run: pattern::normalized(command),
            hides: !balanced || pattern::hides_more(command),
        })
        .collect()
}

/// Whether a deny or ask rule catches the part: the command as run, the
/// path in any case, a searched folder that meets it.
fn restricts(rule: &Rule, part: &Part<'_>, cwd: &str, home: Option<&str>) -> bool {
    let Some(specifier) = &rule.specifier else { return true };
    match part.target {
        Target::Command(_) => pattern::command_matches(specifier, &part.run),
        Target::Path(path) => pattern::path_matches(specifier, path, cwd, home, true),
        Target::Tree(tree) => pattern::tree_meets(specifier, tree, cwd, home),
        Target::Other(subject) => specifier == subject,
    }
}

/// Whether an allow rule covers the part: the command as written (never
/// one hiding another), the path or searched folder exactly.
fn allows(rule: &Rule, part: &Part<'_>, cwd: &str, home: Option<&str>) -> bool {
    let Some(specifier) = &rule.specifier else { return true };
    match part.target {
        Target::Command(_) => !part.hides && pattern::command_matches(specifier, &part.written),
        Target::Path(path) | Target::Tree(path) => pattern::path_matches(specifier, path, cwd, home, false),
        Target::Other(subject) => specifier == subject,
    }
}
