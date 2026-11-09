//! The Superfik client, shared by the desktop and the web app (ADR 0010:
//! a client only shows and asks): the pure reducer ([`state`]) and the
//! view that paints it ([`view`]). No IO here; each shell runs the
//! effects and feeds input in. Builds for wasm32.

pub mod input;
pub mod pages;
mod scenario_ask;
mod scenario_browser;
mod scenario_channels;
mod scenario_claude;
mod scenario_context;
mod scenario_memory;
mod scenario_notes;
mod scenario_org;
pub mod scenario_plugins;
mod scenario_profiles;
mod scenario_routines;
mod scenario_site;
mod scenario_site_memory;
mod scenario_visual;
pub mod scenarios;
pub mod state;
pub mod upload;
pub mod user_settings;
#[allow(
    missing_debug_implementations,
    reason = "views are borrows of the session built and painted within one frame, never kept or logged"
)]
pub mod view;
