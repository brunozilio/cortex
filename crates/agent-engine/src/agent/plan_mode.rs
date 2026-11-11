//! Leaving plan mode with the user's approval (`ExitPlanMode`) and entering
//! it (`EnterPlanMode`). Only the top-level agent changes the mode; the
//! client hears of it through [`EngineEvent::ModeChanged`].

use domain::{AgentAsk, AskId, AskKind, Decision, EngineEvent, PermissionMode};
use serde_json::Value;

use super::Agent;
use crate::tools::Outcome;
use crate::tools::plan_mode::plan;

impl Agent<'_> {
    pub(super) fn exit_plan_mode(&self, id: &str, input: &Value) -> Outcome {
        if self.depth > 0 {
            return Outcome::error("Only the main agent leaves plan mode.");
        }
        if self.control.mode() != PermissionMode::Plan {
            return Outcome::error("You are not in plan mode: there is no plan to approve. Go on with the task.");
        }
        if self.control.no_prompts() {
            return Outcome::error("The plan cannot be put to the user here. Present it as your final message.");
        }
        let Some(plan) = plan(input) else {
            return Outcome::error("Missing the plan: pass it, in markdown, as `plan`.");
        };
        self.notify("plan_approval", concat!(brand::name!(), "'s plan is ready for your review"));
        let ask = AskId(format!("ask-{}-{id}", uuid::Uuid::new_v4()));
        let answer = self.control.open(ask.clone());
        (self.emit)(EngineEvent::Asked(AgentAsk { id: ask, kind: AskKind::Plan { plan: plan.to_owned() } }));
        match answer.recv() {
            Ok(Decision::Approve(mode)) if mode != PermissionMode::Plan => {
                self.control.set_mode(mode);
                let mode = self.control.mode();
                (self.emit)(EngineEvent::ModeChanged(mode));
                let allowed = match mode {
                    PermissionMode::AcceptEdits => "edits run freely, commands ask first",
                    PermissionMode::Bypass => "nothing asks first",
                    PermissionMode::Ask | PermissionMode::Plan => "edits and commands ask the user first",
                };
                Outcome::ok(format!(
                    "The user approved your plan and planning is over ({allowed}). Start implementing it now, \
                     following the plan. Track the steps with the task tools if there are several."
                ))
            }
            Ok(Decision::DenyWith(note)) => Outcome::error(format!(
                "The user wants to keep planning and did not approve the plan. Do not implement anything. They \
                 said: {note}"
            )),
            Ok(
                Decision::Approve(_) | Decision::Deny | Decision::Allow | Decision::AllowAlways | Decision::Answer(_),
            ) => Outcome::error(
                "The user wants to keep planning and did not approve the plan. Do not implement anything. Stop \
                     here and wait for them to say what to change.",
            ),
            Err(_) => Outcome::error("The run was cancelled."),
        }
    }

    pub(super) fn enter_plan_mode(&self) -> Outcome {
        if self.depth > 0 {
            return Outcome::error("Only the main agent enters plan mode.");
        }
        if self.control.mode() == PermissionMode::Plan {
            return Outcome::error("You are already in plan mode.");
        }
        self.control.set_mode(PermissionMode::Plan);
        (self.emit)(EngineEvent::ModeChanged(PermissionMode::Plan));
        Outcome::ok(
            "Plan mode is on. Do not edit files or run commands that change anything: read and search to understand \
             the task, ask the user with AskUserQuestion when a choice is theirs, then present the plan with \
             ExitPlanMode for approval.",
        )
    }
}
