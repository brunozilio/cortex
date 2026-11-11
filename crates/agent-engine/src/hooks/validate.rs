use super::Handler;

const EVENTS: [&str; 13] = [
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
    "PermissionRequest",
    "Notification",
];
const NO_MODEL_HOOKS: [&str; 5] = ["SessionStart", "SessionEnd", "SubagentStart", "PreCompact", "PostCompact"];

pub fn event(event: &str) -> Result<(), String> {
    match EVENTS.contains(&event) {
        true => Ok(()),
        false => Err(format!("unknown hook event {event:?}; the events are {}", EVENTS.join(", "))),
    }
}

pub fn matcher(event: &str, matcher: &str) -> Result<(), String> {
    if matcher.is_empty() || matcher == "*" {
        return Ok(());
    }
    regex::Regex::new(matcher)
        .map(|_| ())
        .map_err(|error| format!("{event}: matcher {matcher:?} is not a regex: {error}"))
}

pub fn hook(event: &str, at: usize, hook: &Handler) -> Result<(), String> {
    let why = match hook.kind.as_str() {
        "command" if hook.command.trim().is_empty() => Some("a command hook needs a `command`".to_owned()),
        "script" if !hook.command.ends_with(".js") => Some("a script hook's `command` must be a .js file".to_owned()),
        "http" if !web_address(hook.url.as_deref()) => {
            Some("an http hook needs a `url` with http or https and no user or password".to_owned())
        }
        "prompt" | "agent" if hook.prompt.as_deref().is_none_or(|p| p.trim().is_empty()) => {
            Some(format!("a {} hook needs a `prompt`", hook.kind))
        }
        "prompt" | "agent" if NO_MODEL_HOOKS.contains(&event) => {
            Some(format!("a {} hook cannot run on {event}; use a command hook", hook.kind))
        }
        "command" | "script" | "http" | "prompt" | "agent" => None,
        other => Some(format!("unknown hook type {other:?}; use command, script, http, prompt or agent")),
    };
    let why = why.or_else(|| {
        (!(1..=600).contains(&hook.timeout)).then(|| format!("timeout {} is outside 1..600 seconds", hook.timeout))
    });
    why.map_or(Ok(()), |why| Err(format!("{event} hook #{}: {why}", at + 1)))
}

fn web_address(url: Option<&str>) -> bool {
    url.and_then(|url| url::Url::parse(url).ok()).is_some_and(|url| {
        matches!(url.scheme(), "http" | "https") && url.username().is_empty() && url.password().is_none()
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::Hooks;

    fn refused(hooks: serde_json::Value) -> String {
        Hooks::parse(&hooks).expect_err("refused")
    }

    #[test]
    fn a_refused_hook_says_which_one_and_why() {
        let ok = json!({"type": "command", "command": "./check.sh"});
        let bad = json!({"type": "prompt", "prompt": "Is it safe?"});
        let why = refused(json!({"SessionStart": [{"hooks": [ok]}, {"hooks": [bad]}]}));
        assert_eq!(why, "SessionStart hook #2: a prompt hook cannot run on SessionStart; use a command hook");
        let slow = json!({"type": "command", "command": "x", "timeout": 0});
        assert!(refused(json!({"Stop": [{"hooks": [slow]}]})).contains("timeout 0 is outside 1..600"));
        assert!(refused(json!({"Stopp": []})).starts_with("unknown hook event \"Stopp\""));
        let odd = json!({"type": "shell", "command": "x"});
        assert!(refused(json!({"Stop": [{"hooks": [odd]}]})).contains("unknown hook type \"shell\""));
        let regex = json!({"PreToolUse": [{"matcher": "(", "hooks": []}]});
        assert!(refused(regex).starts_with("PreToolUse: matcher \"(\" is not a regex"));
    }
}
