//! The browser card as agents use it: what an agent may do to the page
//! beside its conversation, and what it gets back. The page lives with the
//! client; an operation travels to it as a run event and its answer comes
//! back as a command, so the agent always drives the page the user sees.
//! Pure.

use serde::{Deserialize, Serialize};
mod waiting;
pub use waiting::BrowserWait;

pub const BROWSER_SERVER: &str = "superfik_browser";

pub const NAVIGATE_TOOL: &str = "browser_navigate";
pub const BACK_TOOL: &str = "browser_back";
pub const FORWARD_TOOL: &str = "browser_forward";
pub const RELOAD_TOOL: &str = "browser_reload";
pub const SNAPSHOT_TOOL: &str = "browser_snapshot";
pub const CLICK_TOOL: &str = "browser_click";
pub const TYPE_TOOL: &str = "browser_type";
pub const KEY_TOOL: &str = "browser_press_key";
pub const SCROLL_TOOL: &str = "browser_scroll";
pub const SCREENSHOT_TOOL: &str = "browser_screenshot";
pub const EVALUATE_TOOL: &str = "browser_evaluate";
pub const FILL_LOGIN_TOOL: &str = "browser_fill_login";
pub const AUTOFILL_TOOL: &str = "autofill_credential";
pub const WAIT_TOOL: &str = "browser_wait";
pub const TABS_TOOL: &str = "browser_tabs";
pub const SWITCH_TAB_TOOL: &str = "browser_switch_tab";
pub const CLOSE_TAB_TOOL: &str = "browser_close_tab";
pub const FILES_TOOL: &str = "browser_files";
pub const UPLOAD_TOOL: &str = "browser_upload";
pub const DIALOG_TOOL: &str = "browser_dialog";

pub const BROWSER_TOOLS: [&str; 20] = [
    NAVIGATE_TOOL,
    BACK_TOOL,
    FORWARD_TOOL,
    RELOAD_TOOL,
    SNAPSHOT_TOOL,
    CLICK_TOOL,
    TYPE_TOOL,
    KEY_TOOL,
    SCROLL_TOOL,
    SCREENSHOT_TOOL,
    EVALUATE_TOOL,
    FILL_LOGIN_TOOL,
    AUTOFILL_TOOL,
    DIALOG_TOOL,
    WAIT_TOOL,
    TABS_TOOL,
    SWITCH_TAB_TOOL,
    CLOSE_TAB_TOOL,
    FILES_TOOL,
    UPLOAD_TOOL,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserTool {
    pub url: String,
    pub token: String,
}

impl BrowserTool {
    pub fn allowed_tools() -> Vec<String> {
        BROWSER_TOOLS.iter().map(|tool| format!("mcp__{BROWSER_SERVER}__{tool}")).collect()
    }

    pub fn instructions() -> String {
        format!(
            "A browser card sits beside this conversation and the user watches it. Use the \
             mcp__{BROWSER_SERVER}__ tools to browse it: {SNAPSHOT_TOOL} reads the page and numbers what can be \
             clicked or typed into ([ref=N]); act with {CLICK_TOOL}/{TYPE_TOOL} on those refs, then snapshot again. \
             Refs stay stable within a document; read a fresh snapshot after navigation or a stale-ref error. \
             Follow snapshot offset continuations to read long pages. Use {WAIT_TOOL} for bounded page conditions, \
             {TABS_TOOL}/{SWITCH_TAB_TOOL} for native popups, and {FILES_TOOL}/{UPLOAD_TOOL} for staged attachments \
             and download paths. Use {DIALOG_TOOL} to inspect and explicitly respond to JavaScript dialogs. \
             Its cookies are the user's for this session: never change account settings, buy, post or send \
             anything without the user asking. To sign in, use {FILL_LOGIN_TOOL} with the name of a login from \
             the session's vault: the user allows it and the values go into the page without you seeing them; \
             never type a password yourself."
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BrowseId(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Locator {
    Ref(u32),
    Point { x: f32, y: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserOp {
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    Snapshot,
    SnapshotPage { offset: u32 },
    Wait(BrowserWait),
    Tabs,
    Files,
    Dialog { accept: Option<bool>, prompt: Option<String> },
    Upload { reference: u32, paths: Vec<String> },
    SwitchTab { tab: u32 },
    CloseTab { tab: Option<u32> },
    Click { target: Locator },
    Type { target: Option<Locator>, text: String, submit: bool },
    Key { key: String },
    Scroll { dx: i32, dy: i32 },
    Screenshot,
    Evaluate { script: String },
    FillLogin { item: String },
}

impl BrowserOp {
    pub fn verb(&self) -> &'static str {
        match self {
            BrowserOp::Navigate { .. } => "navigate",
            BrowserOp::Back => "back",
            BrowserOp::Forward => "forward",
            BrowserOp::Reload => "reload",
            BrowserOp::Snapshot | BrowserOp::SnapshotPage { .. } => "read",
            BrowserOp::Wait(_) => "wait",
            BrowserOp::Tabs => "tabs",
            BrowserOp::Files => "files",
            BrowserOp::Dialog { .. } => "dialog",
            BrowserOp::Upload { .. } => "upload",
            BrowserOp::SwitchTab { .. } => "switch tab",
            BrowserOp::CloseTab { .. } => "close tab",
            BrowserOp::Click { .. } => "click",
            BrowserOp::Type { .. } => "type",
            BrowserOp::Key { .. } => "key",
            BrowserOp::Scroll { .. } => "scroll",
            BrowserOp::Screenshot => "screenshot",
            BrowserOp::Evaluate { .. } => "evaluate",
            BrowserOp::FillLogin { .. } => "sign in",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BrowserAnswer {
    pub text: String,
    #[serde(default)]
    pub png: Option<String>,
}

impl BrowserAnswer {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), png: None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_allowed_by_its_claude_code_name() {
        let allowed = BrowserTool::allowed_tools();
        assert_eq!(allowed.len(), BROWSER_TOOLS.len());
        assert!(allowed.contains(&"mcp__superfik_browser__browser_click".to_owned()));
    }

    #[test]
    fn operations_travel_as_json() {
        let op = BrowserOp::Type { target: Some(Locator::Ref(3)), text: "hi".into(), submit: true };
        let json = serde_json::to_string(&op).expect("json");
        assert_eq!(serde_json::from_str::<BrowserOp>(&json).expect("back"), op);
    }
}
