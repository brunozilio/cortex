use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::permission::{
    Bounds, Call, Judgement, Place, PolicyContext, PolicyEngine, Remembered, Surroundings, Verdict,
};
use domain::permission_rules::PermissionRules;
use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent, PermissionMode};
use platform::sync::Locked;
use serde_json::Value;

use super::Workspace;
use super::outside::settled;
use super::script_tool::Capability;
use crate::control::Control;

struct Resolving;

impl Surroundings for Resolving {
    fn real_path(&self, path: &Path) -> PathBuf {
        settled(path)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostAsk<'a> {
    pub capability: Capability,
    pub script: &'a str,
    pub arguments: &'a Value,
}

impl HostAsk<'_> {
    fn shown(&self) -> String {
        format!("{}\n(ctx.{} in {})", self.capability.subject(self.arguments), self.capability.name(), self.script)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grounds {
    pub cwd: PathBuf,
    pub home: Option<PathBuf>,
    pub rules: PermissionRules,
    pub bounds: Bounds,
    pub sandboxed: bool,
    pub sandbox_missing: Option<String>,
}

impl Grounds {
    pub fn of(workspace: &Workspace) -> Self {
        Self {
            cwd: workspace.cwd.clone(),
            home: workspace.home.clone(),
            rules: workspace.policy.permissions.clone(),
            bounds: workspace.bounds(),
            sandboxed: workspace.sandboxed(),
            sandbox_missing: workspace.sandbox_refusal(),
        }
    }

    pub fn at(cwd: &Path) -> Self {
        Self::of(&Workspace::new(cwd))
    }

    fn place(&self, path: &str) -> Place {
        let path = Path::new(path);
        let resolved = if path.is_absolute() { path.to_path_buf() } else { self.cwd.join(path) };
        Place { written: super::rule_target::folded(&resolved), real: settled(&resolved) }
    }
}

pub type Emit = Arc<dyn Fn(EngineEvent) + Send + Sync>;

struct Attendant {
    control: Arc<Control>,
    emit: Emit,
}

#[derive(Default)]
pub struct HostJudge {
    attendant: Option<Attendant>,
    grounds: Mutex<Option<Grounds>>,
}

impl std::fmt::Debug for HostJudge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostJudge").field("attended", &self.attendant.is_some()).finish_non_exhaustive()
    }
}

impl HostJudge {
    pub fn unattended() -> Self {
        Self::default()
    }

    pub fn attended(control: Arc<Control>, emit: Emit) -> Self {
        Self { attendant: Some(Attendant { control, emit }), grounds: Mutex::default() }
    }

    pub fn settle(&self, grounds: Grounds) {
        *self.grounds.locked() = Some(grounds);
    }

    pub fn settled(&self) -> Option<Grounds> {
        self.grounds.locked().clone()
    }

    pub fn judge(&self, ask: &HostAsk<'_>, grounds: &Grounds) -> Result<(), String> {
        let tool = ask.capability.tool();
        let subject = ask.capability.subject(ask.arguments);
        let effects = ask.capability.effects(ask.arguments, &|path| grounds.place(path));
        let call = Call { tool, subject: &subject, effects: &effects };
        let remembered = self.attendant.as_ref().map(|attendant| attendant.control.remembered()).unwrap_or_default();
        let judgement = self.judged(&call, grounds, &remembered);
        let refused = |why: &str| format!("ctx.{} in {} was refused: {why}", ask.capability.name(), ask.script);
        match judgement.verdict.clone() {
            Verdict::Allow => Ok(()),
            Verdict::Deny(why) => Err(refused(&why)),
            Verdict::Ask { rememberable, rule, .. } => match self.attendant.as_ref() {
                Some(attendant) if !attendant.control.no_prompts() => {
                    attendant.asked(tool, &ask.shown(), rememberable, rule, &judgement).map_err(|why| refused(&why))
                }
                _ => Err(refused(&nobody_to_ask(tool, rule.as_deref()))),
            },
        }
    }

    fn judged(&self, call: &Call<'_>, grounds: &Grounds, remembered: &Remembered) -> Judgement {
        let (mode, ceiling) = match &self.attendant {
            Some(attendant) => (attendant.control.mode(), attendant.control.ceiling()),
            None => (PermissionMode::Ask, None),
        };
        let cwd = grounds.cwd.to_string_lossy();
        let home = grounds.home.as_ref().map(|home| home.to_string_lossy().into_owned());
        let context = PolicyContext {
            mode,
            ceiling,
            rules: &grounds.rules,
            remembered,
            bounds: &grounds.bounds,
            cwd: &cwd,
            home: home.as_deref(),
            sandboxed: grounds.sandboxed,
            sandbox_missing: grounds.sandbox_missing.as_deref(),
            no_prompts: false,
            surroundings: &Resolving,
        };
        PolicyEngine::judge(call, &context)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SharedJudge(pub Arc<HostJudge>);

impl PartialEq for SharedJudge {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for SharedJudge {}

fn nobody_to_ask(tool: &str, rule: Option<&str>) -> String {
    let allowing =
        rule.map_or_else(|| format!("a permission rule for {tool}"), |rule| format!("the permission rule {rule}"));
    format!(
        "it needs permission and nobody can be asked here. To let it run without a question, add {allowing} to \
         permissions.allow in your settings."
    )
}

impl Attendant {
    fn asked(
        &self,
        tool: &str,
        shown: &str,
        rememberable: bool,
        rule: Option<String>,
        judgement: &Judgement,
    ) -> Result<(), String> {
        let ask = AskId(format!("ask-{}-script", uuid::Uuid::new_v4()));
        let mut answer = self.control.open(ask.clone());
        let kind =
            AskKind::Permission { tool: tool.to_owned(), subject: shown.to_owned(), rememberable, rule: rule.clone() };
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask.clone(), kind }));
        let decision = answer.recv().map_err(|_| "the run stopped while asking the user".to_owned())?;
        (self.emit)(EngineEvent::Answered { ask, decision: decision.clone() });
        match decision {
            Decision::Allow | Decision::Answer(_) | Decision::Approve(_) => Ok(()),
            Decision::AllowAlways => {
                self.control.remember(|remembered| match rule {
                    Some(rule) => remembered.allow_rule(&rule),
                    None => remembered.remember(tool, judgement.standing, &judgement.subject),
                });
                Ok(())
            }
            Decision::Deny => Err("the user did not allow it".to_owned()),
            Decision::DenyWith(note) => Err(format!("the user did not allow it and said: {note}")),
        }
    }
}

#[cfg(test)]
#[path = "host_judge_tests.rs"]
mod tests;
