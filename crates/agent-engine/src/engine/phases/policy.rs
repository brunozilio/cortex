use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use domain::features::{self, Features};
use domain::model_roles::{ModelRole, ModelRoles};
use domain::permission_rules::{PermissionRules, RuleKind};
use domain::{PromptRequest, SessionId};

use super::equip::{Equipped, MainAgent, Toolbox};
use crate::brief::Brief;
use crate::engine::job::Job;
use crate::engine::roles;
use crate::engine::stop::Stop;
use crate::prompt::settings::Settings;
use crate::tools::subagent_config::AgentConfig;
use crate::tools::{self, Policy, Verification, Workspace};

pub(in crate::engine) struct Governed {
    pub(in crate::engine) workspace: Workspace,
    pub(in crate::engine) toolbox: Toolbox,
    pub(in crate::engine) main: MainAgent,
    pub(in crate::engine) roles: ModelRoles,
}

impl Job {
    pub(in crate::engine) fn apply_policy(
        &mut self,
        equipped: Equipped,
        brief: &Brief,
        session: &SessionId,
    ) -> Result<Governed, Stop> {
        let settings = &brief.settings;
        let Equipped { mut workspace, mut toolbox, main } = equipped;
        workspace.policy = flagged(settings, &workspace.cwd, self.request.policy.safety_check);
        workspace.policy.trust = brief.trust;
        if let Some(notice) = sandbox_notice(&workspace, &SANDBOX_SAID) {
            self.gate.emit(domain::EngineEvent::Notice(notice));
        }
        workspace.models.escalation.clone_from(&settings.escalation);
        self.request.policy.budget = budget(&self.request, &settings.features);
        workspace.services.files = Some(Arc::default());
        let roles = roles::merged(&self.request.session.roles, settings);
        if let Some(advisor) = roles.get(ModelRole::Advisor) {
            let model = role_model(&self.request, advisor).map_err(|error| format!("advisorModel: {error}"));
            workspace.models.advisor = Some(model.map_err(Stop::failed)?);
        }
        toolbox.tools.retain(|tool| tool.offered(&workspace));
        workspace.verification = verification(&self.request, settings, &roles).map_err(Stop::failed)?;
        workspace.agent_name = main.name().into();
        workspace.session_id.clone_from(&session.0);
        workspace.anchored_memories = anchored(&self.request, &settings.features);
        workspace.blast_radius = settings.features.on(features::BLAST_RADIUS);
        workspace.hooks = settings.hooks.clone();
        workspace.policy.permissions = permissions(&self.request, settings).map_err(Stop::failed)?;
        Ok(Governed { workspace, toolbox, main, roles })
    }
}

static SANDBOX_SAID: AtomicBool = AtomicBool::new(false);

pub(in crate::engine) fn sandbox_notice(workspace: &Workspace, said: &AtomicBool) -> Option<String> {
    let missing = workspace.sandbox_missing()?;
    (!said.swap(true, Ordering::Relaxed)).then_some(missing.notice)
}

pub(in crate::engine) fn flagged(settings: &Settings, cwd: &Path, safety_check: bool) -> Policy {
    let on = |flag| settings.features.on(flag);
    Policy {
        scripts: settings.scripts,
        sandbox: on(features::SANDBOX).then(|| settings.sandbox.clone()),
        redact: on(features::REDACT_SECRETS).then(|| Arc::new(tools::env_secrets(cwd))),
        snapshots: on(features::DESTRUCTIVE_SNAPSHOT),
        dependency_guard: on(features::DEPENDENCY_GUARD),
        escalation: on(features::ESCALATION),
        auto_review: on(features::AUTO_REVIEW),
        follow_through: on(features::FOLLOW_THROUGH),
        affected_tests: on(features::AFFECTED_TESTS),
        session_claims: on(features::SESSION_CLAIMS),
        load_on_call: on(features::LOAD_ON_CALL),
        code_mode: domain::code_mode::Mode::of(&settings.features),
        teammate_mode: settings.teammate_mode,
        web: domain::web_reading::WebReading::from(&settings.features),
        output: domain::output::Shaping::from(&settings.features),
        bash_output: settings.bash_output,
        safety: safety_check.then(|| Arc::new(crate::guard::Safety::default())),
        ..Policy::default()
    }
}

pub(in crate::engine) fn budget(request: &PromptRequest, features: &Features) -> Option<domain::budget::Budget> {
    request.policy.budget.filter(|_| features.on(features::BUDGET))
}

fn role_model(request: &PromptRequest, name: &str) -> Result<String, String> {
    let config = AgentConfig { model: Some(name.to_owned()), ..Default::default() };
    config.model(&request.session.model.id, request.session.model.provider)
}

pub(in crate::engine) fn verification(
    request: &PromptRequest,
    settings: &Settings,
    roles: &ModelRoles,
) -> Result<Option<Verification>, String> {
    let project = settings
        .project_criteria
        .clone()
        .filter(|_| settings.features.on(features::PROJECT_CRITERIA))
        .map(domain::Criteria::project);
    let asked = request.policy.verification.clone().or(project);
    let Some(criteria) = asked.filter(|_| request.policy.permission != domain::PermissionMode::Plan) else {
        return Ok(None);
    };
    let config = AgentConfig { model: roles.get(ModelRole::Review).map(str::to_owned), ..Default::default() };
    let model = config
        .model(&request.session.model.id, request.session.model.provider)
        .map_err(|error| format!("judgeModel: {error}"))?;
    let allow = criteria.judge_allow(&settings.judge_allow);
    Ok(Some(Verification { criteria, model, allow }))
}

pub(in crate::engine) fn anchored(request: &PromptRequest, features: &Features) -> Vec<domain::Memory> {
    request
        .host
        .memory
        .as_ref()
        .filter(|brief| brief.anchors && features.on(features::MEMORY_ANCHORS))
        .map(|brief| brief.anchored.clone())
        .unwrap_or_default()
}

pub(in crate::engine) fn permissions(request: &PromptRequest, settings: &Settings) -> Result<PermissionRules, String> {
    let mut permissions = settings.permissions.clone();
    let asked = [
        (RuleKind::Allow, &request.policy.allowed_tools),
        (RuleKind::Ask, &request.policy.ask_tools),
        (RuleKind::Deny, &request.policy.disallowed_tools),
    ];
    for (kind, rules) in asked {
        for rule in rules {
            permissions.add(kind, rule).map_err(|error| format!("--allowedTools/--disallowedTools: {error}"))?;
        }
    }
    Ok(permissions)
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
