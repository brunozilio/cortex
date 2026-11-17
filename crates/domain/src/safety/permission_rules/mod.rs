mod hidden;
mod kind;
mod pattern;
mod site;
mod unwrapped;

#[doc(hidden)]
pub use pattern::{command_matches, path_matches};
pub use pattern::{hides_more, normalized, split_command, without_harmless_redirects};
pub use site::{is_private, is_private_address, site};

use crate::tool_name::ToolName;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    Allow,
    Ask,
    Deny,
}

pub type PermissionDecision = RuleKind;

impl RuleKind {
    pub const ALL: [Self; 3] = [Self::Allow, Self::Ask, Self::Deny];

    pub fn word(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }

    pub fn named(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.word() == word)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PermissionsInfo {
    pub rules: Vec<(RuleKind, String)>,
    pub added_dirs: Vec<String>,
    #[serde(default)]
    pub hooks: Vec<HookInfo>,
    #[serde(default)]
    pub notices: Vec<String>,
    #[serde(default)]
    pub skipped_files: Vec<std::path::PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HookInfo {
    pub event: crate::hooks::HookEvent,
    pub matcher: String,
    pub runs: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target<'a> {
    Command(&'a str),
    Path(&'a str),
    Tree(&'a str),
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

    fn names(&self, tool: &str) -> bool {
        if self.tool == tool {
            return true;
        }
        match (ToolName::of(&self.tool), ToolName::of(tool)) {
            (ToolName::Edit, ToolName::Write | ToolName::NotebookEdit) => true,
            (ToolName::Read, ToolName::Glob | ToolName::Grep | ToolName::Lsp) => true,
            (ToolName::Mcp(server), _) if server.matches("__").count() == 1 => tool
                .strip_prefix(&server)
                .and_then(|rest| rest.strip_prefix("__"))
                .is_some_and(|name| !name.is_empty() && !name.contains("__")),
            _ => false,
        }
    }
}

pub fn suggest(tool: &str, subject: &str, outside: bool) -> Option<String> {
    let named = ToolName::of(tool);
    match named {
        ToolName::WebFetch => site(subject).map(|host| format!("{named}(domain:{host})")),
        ToolName::Bash => kind::command_kind(subject),
        ToolName::Read | ToolName::Glob | ToolName::Grep | ToolName::Lsp if outside => {
            let path = subject.trim_end_matches('/');
            let folder = match named {
                ToolName::Read | ToolName::Lsp => path.rsplit_once('/').map(|(dir, _)| dir)?,
                _ => path,
            };
            Some(format!("{}(//{}/**)", ToolName::Read, folder.trim_start_matches('/')))
        }
        _ => None,
    }
}

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

struct Part<'a> {
    target: Target<'a>,
    written: String,
    run: String,
    run_as_written: String,
    hides: bool,
    program: String,
    program_hidden: bool,
    named_later: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionRules {
    rules: Vec<(RuleKind, Rule)>,
}

impl PermissionRules {
    pub fn add(&mut self, kind: RuleKind, text: &str) -> Result<(), String> {
        self.rules.push((kind, Rule::parse(text)?));
        Ok(())
    }

    pub fn with(mut self, kind: RuleKind, text: &str) -> Self {
        if let Ok(rule) = Rule::parse(text) {
            self.rules.push((kind, rule));
        }
        self
    }

    pub fn naming_their_target(&self) -> Self {
        let named = |(kind, rule): &&(RuleKind, Rule)| *kind != RuleKind::Allow || rule.specifier.is_some();
        Self { rules: self.rules.iter().filter(named).cloned().collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    pub fn listed(&self) -> Vec<(RuleKind, String)> {
        let mut listed: Vec<_> = self.rules.iter().map(|(kind, rule)| (*kind, rule.text.clone())).collect();
        listed.sort_by_key(|(kind, _)| match kind {
            RuleKind::Deny => 0,
            RuleKind::Ask => 1,
            RuleKind::Allow => 2,
        });
        listed
    }

    pub fn decide(&self, tool: &str, target: &Target<'_>, cwd: &str, home: Option<&str>) -> Option<RuleKind> {
        self.ruling(tool, target, cwd, home).map(|(kind, _)| kind)
    }

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
        let along = |part: &Part<'_>| matches!(part.target, Target::Command(command) if kind::goes_along(command));
        let covered = parts.iter().all(|part| allowed(part) || along(part)) && parts.iter().any(allowed);
        covered.then(|| (RuleKind::Allow, String::new()))
    }

    pub fn unseen_denial(&self, tool: &str, target: &Target<'_>) -> Option<String> {
        let denials: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|(kind, rule)| *kind == RuleKind::Deny && rule.names(tool) && rule.specifier.is_some())
            .map(|(_, rule)| rule)
            .collect();
        if denials.is_empty() {
            return None;
        }
        parts(target).iter().find_map(|part| {
            let named_later = |rule: &Rule| rule.program().is_some_and(|named| part.named_later.contains(&named));
            let could_be = |rule: &Rule| {
                (part.hides && (part.program_hidden || rule.program().is_none_or(|named| named == part.program)))
                    || named_later(rule)
            };
            denials.iter().find(|rule| could_be(rule)).map(|rule| rule.text.clone())
        })
    }
}

impl Rule {
    fn program(&self) -> Option<String> {
        let specifier = self.specifier.as_deref()?;
        let specifier = specifier.strip_suffix(":*").unwrap_or(specifier);
        let first = unwrapped::shell_words(specifier).into_iter().next()?;
        let named = unwrapped::program_name(&first);
        (!named.is_empty() && !named.contains(['*', '?'])).then_some(named)
    }
}

fn parts<'a>(target: &Target<'a>) -> Vec<Part<'a>> {
    let Target::Command(line) = target else {
        let part = Part {
            target: *target,
            written: String::new(),
            run: String::new(),
            run_as_written: String::new(),
            hides: false,
            program: String::new(),
            program_hidden: false,
            named_later: Vec::new(),
        };
        return vec![part];
    };
    let (commands, balanced) = pattern::split_command(line);
    let mut found = Vec::new();
    for command in commands {
        for code in add_part(&mut found, Target::Command(command), command, balanced, false) {
            add_inner_parts(&mut found, line, &code, 1);
        }
    }
    found
}

fn add_part<'a>(
    found: &mut Vec<Part<'a>>,
    target: Target<'a>,
    command: &str,
    balanced: bool,
    inner: bool,
) -> Vec<String> {
    let bare = crate::shell::SimpleCommand::bare(command);
    let seen = hidden::seen(&bare, balanced);
    found.push(Part {
        target,
        written: pattern::collapsed(command),
        run: pattern::normalized(command),
        run_as_written: pattern::as_written(command),
        hides: inner || !balanced || seen.hidden || pattern::hides_more(command),
        program: seen.program,
        program_hidden: seen.hidden,
        named_later: seen.named_later,
    });
    seen.code
}

fn add_inner_parts<'a>(found: &mut Vec<Part<'a>>, line: &'a str, code: &str, depth: usize) {
    let (commands, balanced) = pattern::split_command(code);
    for command in commands {
        let codes = add_part(found, Target::Command(line), command, balanced, true);
        if !codes.is_empty() && hidden::deep_enough(depth) {
            if let Some(last) = found.last_mut() {
                last.program_hidden = true;
            }
            continue;
        }
        for code in codes {
            add_inner_parts(found, line, &code, depth + 1);
        }
    }
}

fn restricts(rule: &Rule, part: &Part<'_>, cwd: &str, home: Option<&str>) -> bool {
    let Some(specifier) = &rule.specifier else { return true };
    match part.target {
        Target::Command(_) => {
            let folded = |text: &str| unwrapped::unquoted(text).to_lowercase();
            [&part.run, &part.run_as_written].into_iter().any(|run| {
                pattern::command_matches(specifier, run)
                    || pattern::command_matches(&unwrapped::unquoted(specifier), &unwrapped::unquoted(run))
                    || pattern::command_matches(&folded(specifier), &folded(run))
            })
        }
        Target::Path(path) => pattern::path_matches(specifier, path, cwd, home, true),
        Target::Tree(tree) => pattern::tree_meets(specifier, tree, cwd, home),
        Target::Other(subject) => specifier == subject,
    }
}

fn allows(rule: &Rule, part: &Part<'_>, cwd: &str, home: Option<&str>) -> bool {
    let Some(specifier) = &rule.specifier else { return true };
    match part.target {
        Target::Command(_) => !part.hides && pattern::command_matches(specifier, &part.written),
        Target::Path(path) | Target::Tree(path) => pattern::path_matches(specifier, path, cwd, home, false),
        Target::Other(subject) => specifier == subject,
    }
}
