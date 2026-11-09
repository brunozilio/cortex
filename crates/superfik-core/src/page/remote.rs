//! A page on the server: the calls a client sends it and what it reports.

use serde::{Deserialize, Serialize};

use super::{PageCursor, PageInput, Profile};
use crate::{BrowseId, BrowserAnswer, BrowserId, BrowserOp};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageCall {
    Open { id: BrowserId, url: String, profile: Profile, size: (u32, u32), scale: f32 },
    Close(BrowserId),
    Navigate(BrowserId, String),
    Back(BrowserId),
    Forward(BrowserId),
    Reload(BrowserId),
    Stop(BrowserId),
    Resize { id: BrowserId, size: (u32, u32), scale: f32 },
    Focus(BrowserId, bool),
    Input(BrowserId, PageInput),
    ClearCookies(BrowserId),
    Agent { id: BrowserId, request: BrowseId, op: BrowserOp },
    Refresh(BrowserId),
}

impl PageCall {
    pub fn page(&self) -> BrowserId {
        match self {
            Self::Open { id, .. } | Self::Resize { id, .. } | Self::Agent { id, .. } => *id,
            Self::Close(id)
            | Self::Navigate(id, _)
            | Self::Back(id)
            | Self::Forward(id)
            | Self::Reload(id)
            | Self::Stop(id)
            | Self::Focus(id, _)
            | Self::Input(id, _)
            | Self::ClearCookies(id)
            | Self::Refresh(id) => *id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageReport {
    Address(String),
    Title(String),
    Loading(bool),
    History { back: bool, forward: bool },
    Failed { url: String, error: String },
    Crashed,
    Copied(String),
    Cursor(PageCursor),
    Answered { request: BrowseId, answer: Result<BrowserAnswer, String> },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_call_names_its_page_and_survives_the_wire() {
        let id = BrowserId::new();
        let calls = [
            PageCall::Open {
                id,
                url: "https://example.com".into(),
                profile: Profile::Persistent("c1".into()),
                size: (640, 400),
                scale: 2.0,
            },
            PageCall::Input(id, PageInput::Text("hé".into())),
            PageCall::Agent { id, request: BrowseId("b1".into()), op: BrowserOp::Back },
            PageCall::Refresh(id),
        ];
        for call in calls {
            assert_eq!(call.page(), id);
            let json = serde_json::to_string(&call).expect("serialises");
            assert_eq!(serde_json::from_str::<PageCall>(&json).expect("parses"), call);
        }
    }
}
