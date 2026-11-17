#![allow(clippy::expect_used, clippy::panic, reason = "a failed setup step stops the test")]
#![allow(clippy::too_many_lines, reason = "a golden transcript is a literal with one line per event")]

#[path = "golden/always_allow.rs"]
mod always_allow;
#[path = "golden/cancellation.rs"]
mod cancellation;
#[path = "golden/compaction.rs"]
mod compaction;
#[path = "golden/cutoffs.rs"]
mod cutoffs;
#[cfg(unix)]
#[path = "golden/hooks.rs"]
mod hooks;
#[path = "golden/model_errors.rs"]
mod model_errors;
#[path = "golden/models.rs"]
mod models;
#[path = "golden/permissions.rs"]
mod permissions;
#[path = "golden/plan_mode.rs"]
mod plan_mode;
#[path = "golden/render.rs"]
mod render;
#[path = "golden/run.rs"]
mod run;
#[path = "golden/sessions.rs"]
mod sessions;
#[path = "golden/stage.rs"]
mod stage;
#[path = "golden/steering.rs"]
mod steering;
#[cfg(unix)]
#[path = "golden/stop_hooks.rs"]
mod stop_hooks;
#[path = "golden/structured.rs"]
mod structured;
#[path = "golden/sub_agents.rs"]
mod sub_agents;
#[cfg(unix)]
#[path = "golden/trust.rs"]
mod trust;
#[path = "golden/turns.rs"]
mod turns;
#[cfg(unix)]
#[path = "golden/worktrees.rs"]
mod worktrees;
