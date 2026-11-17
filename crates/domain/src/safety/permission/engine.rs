use std::path::Path;

use super::rules::{Ruling, install_rule, install_ruling, rule_for, suggested, unseen_denial};
use super::{Call, Effect, Exec, Guard, Judgement, Place, PolicyContext, Reason, Remembered, Standing, Verdict};
use crate::PermissionMode;
use crate::command_policy::{
    CommandClass, GitFacts, classify_command, paths_read, paths_read_anywhere, sign_in_refused,
};
use crate::permission_rules::RuleKind;
use crate::tool_name::ToolName;

#[derive(Debug, Clone, Copy)]
pub struct PolicyEngine;

struct Stood {
    standing: Standing,
    subject: String,
    guarded: Option<(std::path::PathBuf, Guard)>,
}

impl PolicyEngine {
    pub fn decide(call: &Call<'_>, cx: &PolicyContext<'_>) -> Verdict {
        Self::judge(call, cx).verdict
    }

    pub fn judge(call: &Call<'_>, cx: &PolicyContext<'_>) -> Judgement {
        let Classes { each: classes, read_outside, read_anywhere } = classes(call, cx);
        let reads = read_by_commands(call, &read_outside);
        let as_read = Call { tool: "Read", subject: call.subject, effects: &reads };
        let stood = standing(call, cx, &classes, read_outside.first());
        let rule = if reads.is_empty() {
            suggested(call, &stood.subject, stood.standing == Standing::ReadsOutside)
        } else {
            suggested(&as_read, &stood.subject, true)
        };
        let judged = |verdict| Judgement {
            verdict,
            standing: stood.standing,
            subject: stood.subject.clone(),
            guarded: stood.guarded.clone(),
            rule: rule.clone(),
        };
        if let Some(refusal) = forbidden(call, cx, &classes) {
            return judged(Verdict::Deny(refusal));
        }
        let mode = cx.mode();
        let by_mode = by_mode(mode, stood.standing, call.tool, &stood.subject, cx.remembered);
        let ruling = rule_for(call, cx, stood.guarded.is_some());
        let ruling = if reads.is_empty() { ruling } else { stricter(ruling, rule_for(&as_read, cx, false)) };
        let ruling = harsher(ruling, read_rule(call, cx, &read_anywhere));
        let asked_by = match &ruling {
            Some((RuleKind::Ask, text)) => Some(text.clone()),
            _ => None,
        };
        let verdict = match ruled(mode, stood.standing, ruling, by_mode) {
            Verdict::Deny(reason) => Verdict::Deny(reason),
            settled => {
                let asks = settled.asks();
                let unseen = unseen_denial(call, cx).map(Reason::Hidden);
                let concern = unseen.or_else(|| cx.concern(call, risk(&classes), leaves_the_sandbox(call, cx)));
                let reason = concern.or_else(|| install_concern(call, cx)).or_else(|| {
                    asks.then(|| match (asked_by, &stood.guarded, stood.standing) {
                        (Some(rule), ..) => Reason::Rule(rule),
                        (None, Some((path, guard)), _) => Reason::Guarded { path: path.clone(), guard: *guard },
                        (None, None, Standing::ReadsOutside) => Reason::Outside(stood.subject.clone().into()),
                        (None, None, _) => Reason::Mode,
                    })
                });
                match reason {
                    Some(reason) => Verdict::Ask { rememberable: reason.rememberable(), reason, rule: rule.clone() },
                    None => Verdict::Allow,
                }
            }
        };
        judged(verdict)
    }
}

fn execs<'a>(call: &Call<'a>) -> impl Iterator<Item = &'a Exec> {
    call.effects.iter().filter_map(|effect| match effect {
        Effect::Exec(exec) => Some(exec),
        _ => None,
    })
}

struct Classes {
    each: Vec<CommandClass>,
    read_outside: Vec<Place>,
    read_anywhere: Vec<Effect>,
}

fn classes(call: &Call<'_>, cx: &PolicyContext<'_>) -> Classes {
    let mut read_outside = Vec::new();
    let mut read_anywhere = Vec::new();
    let each = execs(call)
        .map(|exec| {
            let branches =
                if exec.command.text().contains("push") { cx.surroundings.branches() } else { Default::default() };
            let git =
                GitFacts { current_branch: branches.current.as_deref(), default_branch: branches.default.as_deref() };
            let class = classify_command(&exec.command, &git, &|path| cx.bounds.may_read(path));
            for operand in paths_read(&exec.command, cx.cwd, cx.home) {
                let real = cx.surroundings.real_path(&operand.path);
                let place = Place { written: operand.path, real };
                if !cx.bounds.may_read(&place.real) && !read_outside.contains(&place) {
                    read_outside.push(place.clone());
                }
                read_anywhere.push(if operand.tree { Effect::ReadTree(place) } else { Effect::ReadPath(place) });
            }
            if class != CommandClass::Unknown {
                return class;
            }
            match paths_read_anywhere(&exec.command, cx.home) {
                Some(paths) => {
                    let outside = paths.into_iter().filter(|path| !cx.bounds.may_read(path));
                    read_outside.extend(outside.map(Place::at));
                    CommandClass::ReadOnly
                }
                None => class,
            }
        })
        .collect();
    Classes { each, read_outside, read_anywhere }
}

fn read_rule(call: &Call<'_>, cx: &PolicyContext<'_>, reads: &[Effect]) -> Ruling {
    if reads.is_empty() || ToolName::of(call.tool) != ToolName::Bash {
        return None;
    }
    let as_read = Call { tool: ToolName::Read.as_str(), subject: call.subject, effects: reads };
    rule_for(&as_read, cx, false).filter(|(kind, _)| *kind != RuleKind::Allow)
}

fn harsher(first: Ruling, then: Ruling) -> Ruling {
    let rank = |ruling: &Ruling| match ruling {
        Some((RuleKind::Deny, _)) => 3,
        Some((RuleKind::Ask, _)) => 2,
        Some((RuleKind::Allow, _)) => 1,
        None => 0,
    };
    if rank(&then) > rank(&first) { then } else { first }
}

fn read_by_commands(call: &Call<'_>, read_outside: &[Place]) -> Vec<Effect> {
    if ToolName::of(call.tool) == ToolName::Bash {
        read_outside.iter().cloned().map(Effect::ReadPath).collect()
    } else {
        Vec::new()
    }
}

fn stricter(first: Ruling, then: Ruling) -> Ruling {
    match (&first, &then) {
        (_, Some((RuleKind::Deny, _))) if !matches!(first, Some((RuleKind::Deny, _))) => then,
        (Some(_), _) => first,
        (None, _) => then,
    }
}

fn installs(call: &Call<'_>) -> Option<(String, String)> {
    call.effects.iter().find_map(|effect| match effect {
        Effect::Install { server, what } => Some((server.clone(), what.clone())),
        _ => None,
    })
}

fn install_refused(call: &Call<'_>, cx: &PolicyContext<'_>) -> Option<String> {
    let (server, _) = installs(call)?;
    if cx.mode() == PermissionMode::Plan {
        return Some(format!(
            "Installing the {server} language server is not available while planning: only read and search now, \
             then present the plan."
        ));
    }
    (install_ruling(call, cx) == Some(RuleKind::Deny)).then(|| {
        format!(
            "The permission rule {} in settings refuses installing it. Do not retry it; take another approach or \
             ask the user.",
            install_rule(call.tool, &server)
        )
    })
}

fn install_concern(call: &Call<'_>, cx: &PolicyContext<'_>) -> Option<Reason> {
    let (server, what) = installs(call)?;
    (install_ruling(call, cx) != Some(RuleKind::Allow))
        .then(|| Reason::Install { what, rule: install_rule(call.tool, &server) })
}

fn forbidden(call: &Call<'_>, cx: &PolicyContext<'_>, classes: &[CommandClass]) -> Option<String> {
    if let Some(refused) = install_refused(call, cx) {
        return Some(refused);
    }
    if let Some(why) = cx.sandbox_missing
        && execs(call).any(|exec| !exec.unsandboxed)
    {
        return Some(crate::sandbox::refusal(why));
    }
    let signing_in =
        execs(call).find_map(|exec| sign_in_refused(exec.command.text(), cx.sandboxed && !exec.unsandboxed));
    signing_in.or_else(|| {
        classes.iter().find_map(|class| match class {
            CommandClass::Forbidden(reason) => Some(reason.clone()),
            _ => None,
        })
    })
}

fn risk(classes: &[CommandClass]) -> Option<String> {
    classes.iter().find_map(|class| match class {
        CommandClass::Risky(why) => Some(why.clone()),
        _ => None,
    })
}

fn leaves_the_sandbox(call: &Call<'_>, cx: &PolicyContext<'_>) -> bool {
    (cx.sandboxed || cx.sandbox_missing.is_some()) && execs(call).any(|exec| exec.unsandboxed)
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

fn standing(call: &Call<'_>, cx: &PolicyContext<'_>, classes: &[CommandClass], read: Option<&Place>) -> Stood {
    let outside = call
        .effects
        .iter()
        .find_map(|effect| match effect {
            Effect::ReadPath(place) | Effect::ReadTree(place) if !cx.bounds.may_read(&place.real) => Some(place),
            _ => None,
        })
        .or(read);
    let guarded = call.effects.iter().find_map(|effect| match effect {
        Effect::WritePath(place) => cx.bounds.guard(&place.real).map(|guard| (place.real.clone(), guard)),
        _ => None,
    });
    let plain = |standing| Stood { standing, subject: call.subject.to_owned(), guarded: None };
    let mut read_only = classes.iter().map(|class| *class == CommandClass::ReadOnly);
    let acts = call.effects.iter().any(|effect| match effect {
        Effect::Exec(_) => !read_only.next().unwrap_or(false),
        Effect::Install { .. } => false,
        other => other.acts(),
    });
    if acts {
        return plain(Standing::Acts);
    }
    match (outside, guarded) {
        (Some(place), _) => Stood { standing: Standing::ReadsOutside, subject: shown(&place.written), guarded: None },
        (None, Some((path, guard))) => {
            Stood { standing: Standing::EditsGuarded, subject: shown(&path), guarded: Some((path, guard)) }
        }
        (None, None) if call.effects.iter().any(Effect::changes_files) => plain(Standing::Edits),
        (None, None) => plain(Standing::Reads),
    }
}

fn by_mode(mode: PermissionMode, standing: Standing, tool: &str, subject: &str, remembered: &Remembered) -> Verdict {
    let ask = || Verdict::Ask { reason: Reason::Mode, rememberable: true, rule: None };
    match (mode, standing) {
        (_, Standing::Reads) | (PermissionMode::Bypass, _) => Verdict::Allow,
        (_, Standing::ReadsOutside) if remembered.allows(tool, standing, subject) => Verdict::Allow,
        (_, Standing::ReadsOutside) => ask(),
        (PermissionMode::Plan, _) => Verdict::Deny(format!(
            "{tool} is not available while planning: only read and search now, then present the plan."
        )),
        (PermissionMode::AcceptEdits, Standing::Edits) => Verdict::Allow,
        _ if remembered.allows(tool, standing, subject) => Verdict::Allow,
        _ => ask(),
    }
}

fn ruled(mode: PermissionMode, standing: Standing, rule: Option<(RuleKind, String)>, by_mode: Verdict) -> Verdict {
    let planning = mode == PermissionMode::Plan && standing.refused_while_planning();
    match rule {
        Some((RuleKind::Deny, rule)) => Verdict::Deny(format!(
            "The permission rule {rule} in settings refuses this. Do not retry it; take another approach or ask the user."
        )),
        _ if mode == PermissionMode::Bypass || planning => by_mode,
        Some((RuleKind::Ask, _)) => Verdict::Ask { reason: Reason::Mode, rememberable: true, rule: None },
        Some((RuleKind::Allow, _)) => Verdict::Allow,
        None => by_mode,
    }
}
