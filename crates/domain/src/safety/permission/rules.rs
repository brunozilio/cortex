use std::path::Path;

use super::{Call, Effect, PolicyContext};
use crate::permission_rules::{PermissionRules, RuleKind, Target, suggest};
use crate::tool_name::ToolName;

pub type Ruling = Option<(RuleKind, String)>;

struct Judged {
    ruling: Ruling,
    restricts_only: bool,
}

pub fn rule_for(call: &Call<'_>, cx: &PolicyContext<'_>, guarded: bool) -> Ruling {
    let recalled = cx.remembered.rules();
    let within = |rules: &PermissionRules| ruling_in(rules, call, cx.cwd, cx.home);
    if guarded {
        return within(&cx.rules.naming_their_target()).or_else(|| within(&recalled.naming_their_target()));
    }
    within(cx.rules).or_else(|| within(recalled))
}

pub fn ruling_in(rules: &PermissionRules, call: &Call<'_>, cwd: &str, home: Option<&str>) -> Ruling {
    if rules.is_empty() {
        return None;
    }
    let rule = |tool: &str, target: &Target<'_>| rules.ruling(tool, target, cwd, home);
    let at = |path: &Path, tree: bool| {
        let path = path.to_string_lossy();
        rule(call.tool, &if tree { Target::Tree(&path) } else { Target::Path(&path) })
    };
    let own = |ruling| Judged { ruling, restricts_only: false };
    let mut judged: Vec<Judged> = Vec::new();
    for effect in call.effects {
        match effect {
            Effect::ReadPath(place) | Effect::WritePath(place) | Effect::ReadTree(place) => {
                let tree = matches!(effect, Effect::ReadTree(_));
                judged.push(own(at(&place.written, tree)));
                if place.real != place.written {
                    judged.push(own(at(&place.real, tree)));
                }
            }
            Effect::Exec(exec) => judged.push(Judged {
                ruling: rule(ToolName::Bash.as_str(), &Target::Command(exec.command.text())),
                restricts_only: ToolName::of(call.tool) != ToolName::Bash,
            }),
            Effect::Net { host: Some(host) } => {
                judged.push(own(rule(call.tool, &Target::Other(&format!("domain:{host}")))));
            }
            Effect::Resource { server, uri } => {
                judged.push(own(rule(&format!("mcp__{server}__resources"), &Target::Other(uri))));
            }
            Effect::Net { host: None }
            | Effect::Spawn(_)
            | Effect::Cancel { .. }
            | Effect::Notify
            | Effect::Session { .. }
            | Effect::Remote { .. }
            | Effect::Install { .. }
            | Effect::Declared(_) => {}
        }
    }
    if judged.iter().all(|judged| judged.restricts_only) {
        judged.push(own(rule(call.tool, &Target::Other(call.subject))));
    }
    strictest(judged)
}

fn strictest(judged: Vec<Judged>) -> Ruling {
    for wanted in [RuleKind::Deny, RuleKind::Ask] {
        let found = judged.iter().find_map(|judged| judged.ruling.clone().filter(|(kind, _)| *kind == wanted));
        if found.is_some() {
            return found;
        }
    }
    let mut needed = judged.into_iter().filter(|judged| !judged.restricts_only);
    let first = needed.next()?.ruling?;
    needed.all(|judged| judged.ruling.is_some()).then_some(first)
}

pub fn suggested(call: &Call<'_>, subject: &str, outside: bool) -> Option<String> {
    let from_effect = call.effects.iter().find_map(|effect| match effect {
        Effect::Exec(exec) if ToolName::of(call.tool) == ToolName::Bash => {
            Some(suggest(call.tool, exec.command.text(), false))
        }
        Effect::Net { host: Some(host) } => Some(Some(format!("{}(domain:{host})", call.tool))),
        Effect::Install { server, .. } => Some(Some(install_rule(call.tool, server))),
        _ => None,
    });
    from_effect.unwrap_or_else(|| suggest(call.tool, subject, outside))
}

pub(super) fn unseen_denial(call: &Call<'_>, cx: &PolicyContext<'_>) -> Option<String> {
    let recalled = cx.remembered.rules();
    call.effects.iter().find_map(|effect| match effect {
        Effect::Exec(exec) => {
            let target = Target::Command(exec.command.text());
            let tool = ToolName::Bash.as_str();
            cx.rules.unseen_denial(tool, &target).or_else(|| recalled.unseen_denial(tool, &target))
        }
        _ => None,
    })
}

pub fn install_rule(tool: &str, server: &str) -> String {
    format!("{tool}(install:{server})")
}

pub(super) fn install_ruling(call: &Call<'_>, cx: &PolicyContext<'_>) -> Option<RuleKind> {
    let recalled = cx.remembered.rules();
    let rulings = call.effects.iter().filter_map(|effect| match effect {
        Effect::Install { server, .. } => {
            let target = format!("install:{server}");
            let ruled = |rules: &PermissionRules| {
                rules.naming_their_target().decide(call.tool, &Target::Other(&target), cx.cwd, cx.home)
            };
            Some(ruled(cx.rules).or_else(|| ruled(recalled)))
        }
        _ => None,
    });
    let rulings: Vec<Option<RuleKind>> = rulings.collect();
    if rulings.is_empty() {
        None
    } else if rulings.contains(&Some(RuleKind::Deny)) {
        Some(RuleKind::Deny)
    } else if rulings.iter().all(|ruling| *ruling == Some(RuleKind::Allow)) {
        Some(RuleKind::Allow)
    } else {
        Some(RuleKind::Ask)
    }
}
