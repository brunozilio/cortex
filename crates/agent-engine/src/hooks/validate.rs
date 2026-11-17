use domain::hooks::{HandlerKind, HookEvent};
use serde_json::Value;

use super::Handler;

pub(crate) const TIMEOUT_SECONDS: std::ops::RangeInclusive<u64> = 1..=600;

pub fn named(written: &Value) -> Result<(), String> {
    let Some(events) = written.as_object() else { return Ok(()) };
    for (event, groups) in events {
        if HookEvent::named(event).is_none() {
            let known = HookEvent::ALL.map(HookEvent::as_str).join(", ");
            return Err(format!("unknown hook event {event:?}; the events are {known}"));
        }
        let handlers = groups.as_array().into_iter().flatten().flat_map(|group| group["hooks"].as_array()).flatten();
        for (at, handler) in handlers.enumerate() {
            if let Some(kind) = handler["type"].as_str().filter(|kind| HandlerKind::named(kind).is_none()) {
                return Err(format!(
                    "{event} hook #{}: unknown hook type {kind:?}; use command, script, http, prompt or agent",
                    at + 1
                ));
            }
        }
    }
    Ok(())
}

pub fn matcher(event: HookEvent, matcher: &str) -> Result<(), String> {
    if matcher.is_empty() || matcher == "*" {
        return Ok(());
    }
    regex::Regex::new(matcher)
        .map(|_| ())
        .map_err(|error| format!("{event}: matcher {matcher:?} is not a regex: {error}"))
}

pub fn hook(event: HookEvent, at: usize, hook: &Handler) -> Result<(), String> {
    let kind = hook.kind;
    let why = match kind {
        HandlerKind::Command if hook.command.trim().is_empty() => Some("a command hook needs a `command`".to_owned()),
        HandlerKind::Script if !hook.command.ends_with(".js") => {
            Some("a script hook's `command` must be a .js file".to_owned())
        }
        HandlerKind::Http if !web_address(hook.url.as_deref()) => {
            Some("an http hook needs a `url` with http or https and no user or password".to_owned())
        }
        HandlerKind::Prompt | HandlerKind::Agent if hook.prompt.as_deref().is_none_or(|p| p.trim().is_empty()) => {
            Some(format!("a {kind} hook needs a `prompt`"))
        }
        HandlerKind::Prompt | HandlerKind::Agent if !event.may_ask_a_model() => {
            Some(format!("a {kind} hook cannot run on {event}; use a command hook"))
        }
        _ => None,
    };
    let why = why.or_else(|| {
        (!TIMEOUT_SECONDS.contains(&hook.timeout))
            .then(|| format!("timeout {} is outside 1..600 seconds", hook.timeout))
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
        for event in ["RunStart", "RunEnd"] {
            let asked = json!({"type": "prompt", "prompt": "Is it safe?"});
            let why = refused(json!({ event: [{"hooks": [asked]}] }));
            assert_eq!(why, format!("{event} hook #1: a prompt hook cannot run on {event}; use a command hook"));
        }
        let slow = json!({"type": "command", "command": "x", "timeout": 0});
        assert!(refused(json!({"Stop": [{"hooks": [slow]}]})).contains("timeout 0 is outside 1..600"));
        assert!(refused(json!({"Stopp": []})).starts_with("unknown hook event \"Stopp\""));
        let odd = json!({"type": "shell", "command": "x"});
        assert_eq!(
            refused(json!({"Stop": [{"hooks": [odd]}]})),
            "Stop hook #1: unknown hook type \"shell\"; use command, script, http, prompt or agent"
        );
        let regex = json!({"PreToolUse": [{"matcher": "(", "hooks": []}]});
        assert!(refused(regex).starts_with("PreToolUse: matcher \"(\" is not a regex"));
    }

    #[test]
    fn a_status_message_is_accepted_and_not_kept() {
        let told = json!({"type": "command", "command": "./check.sh", "status_message": "Checking"});
        let plain = json!({"type": "command", "command": "./check.sh"});
        let hooks = Hooks::parse(&json!({"Stop": [{"hooks": [told]}]})).expect("accepted");
        assert_eq!(hooks, Hooks::parse(&json!({"Stop": [{"hooks": [plain]}]})).expect("plain"));
        assert!(!serde_json::to_string(&hooks).expect("json").contains("status_message"));
        let unknown = json!({"type": "command", "command": "x", "status": "Checking"});
        assert!(refused(json!({"Stop": [{"hooks": [unknown]}]})).contains("unknown field"));
    }
}
