//! The seam between the app and a browser engine.

use superfik_core::page::PageCall;
use superfik_core::{BrowseId, BrowserId, BrowserOp};

use crate::{PageEvent, PageInput, Profile};

#[derive(Debug, Clone, PartialEq)]
pub enum Call {
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
    SetCookies { id: BrowserId, cookies: Vec<crate::import::Cookie> },
    ClearCookies(BrowserId),
    FillLogin { id: BrowserId, url: String, fill: superfik_core::autofill::LoginFill },
    AnswerPasskey { id: BrowserId, request: u64, reply: crate::passkey::PasskeyReply },
    Agent { id: BrowserId, request: BrowseId, op: BrowserOp },
}

impl Call {
    pub fn remote(self) -> Option<PageCall> {
        Some(match self {
            Self::Open { id, url, profile, size, scale } => PageCall::Open { id, url, profile, size, scale },
            Self::Close(id) => PageCall::Close(id),
            Self::Navigate(id, url) => PageCall::Navigate(id, url),
            Self::Back(id) => PageCall::Back(id),
            Self::Forward(id) => PageCall::Forward(id),
            Self::Reload(id) => PageCall::Reload(id),
            Self::Stop(id) => PageCall::Stop(id),
            Self::Resize { id, size, scale } => PageCall::Resize { id, size, scale },
            Self::Focus(id, focused) => PageCall::Focus(id, focused),
            Self::Input(id, input) => PageCall::Input(id, input),
            Self::ClearCookies(id) => PageCall::ClearCookies(id),
            Self::Agent { id, request, op } => PageCall::Agent { id, request, op },
            Self::SetCookies { .. } | Self::FillLogin { .. } | Self::AnswerPasskey { .. } => return None,
        })
    }

    pub fn from_remote(call: PageCall) -> Option<Self> {
        Some(match call {
            PageCall::Open { id, url, profile, size, scale } => Self::Open { id, url, profile, size, scale },
            PageCall::Close(id) => Self::Close(id),
            PageCall::Navigate(id, url) => Self::Navigate(id, url),
            PageCall::Back(id) => Self::Back(id),
            PageCall::Forward(id) => Self::Forward(id),
            PageCall::Reload(id) => Self::Reload(id),
            PageCall::Stop(id) => Self::Stop(id),
            PageCall::Resize { id, size, scale } => Self::Resize { id, size, scale },
            PageCall::Focus(id, focused) => Self::Focus(id, focused),
            PageCall::Input(id, input) => Self::Input(id, input),
            PageCall::ClearCookies(id) => Self::ClearCookies(id),
            PageCall::Agent { id, request, op } => Self::Agent { id, request, op },
            PageCall::Refresh(_) => return None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub view: wgpu::TextureView,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub generation: u64,
}

pub trait BrowserHost {
    fn warm_up(&mut self) {}

    fn send(&mut self, call: Call);

    fn pump(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<(BrowserId, PageEvent)>;

    fn frame(&self, id: BrowserId) -> Option<Frame>;

    fn is_active(&self) -> bool;

    fn suspend(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct Unavailable {
    reason: String,
    pending: Vec<(BrowserId, PageEvent)>,
}

impl Default for Unavailable {
    fn default() -> Self {
        Self::because("The browser engine is unavailable.")
    }
}

impl Unavailable {
    pub fn because(reason: impl Into<String>) -> Self {
        Self { reason: reason.into(), pending: Vec::new() }
    }
}

impl BrowserHost for Unavailable {
    fn send(&mut self, call: Call) {
        match call {
            Call::Open { id, url, .. } | Call::Navigate(id, url) => {
                self.pending.push((id, PageEvent::Failed { url, error: self.reason.clone() }));
                self.pending.push((id, PageEvent::Loading(false)));
            }
            Call::Agent { id, request, .. } => {
                self.pending.push((id, PageEvent::Answered { request, answer: Err(self.reason.clone()) }));
            }
            _ => {}
        }
    }

    fn pump(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) -> Vec<(BrowserId, PageEvent)> {
        std::mem::take(&mut self.pending)
    }

    fn frame(&self, _id: BrowserId) -> Option<Frame> {
        None
    }

    fn is_active(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct Recorder {
    pub calls: Vec<Call>,
    pub events: Vec<(BrowserId, PageEvent)>,
}

impl BrowserHost for Recorder {
    fn send(&mut self, call: Call) {
        self.calls.push(call);
    }

    fn pump(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue) -> Vec<(BrowserId, PageEvent)> {
        std::mem::take(&mut self.events)
    }

    fn frame(&self, _id: BrowserId) -> Option<Frame> {
        None
    }

    fn is_active(&self) -> bool {
        let opened = self.calls.iter().filter(|c| matches!(c, Call::Open { .. })).count();
        let closed = self.calls.iter().filter(|c| matches!(c, Call::Close(_))).count();
        opened > closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calls_that_travel_come_back_unchanged_and_vault_fills_never_leave() {
        let id = BrowserId::new();
        let open =
            Call::Open { id, url: "https://a.test".into(), profile: Profile::Ephemeral, size: (8, 8), scale: 2.0 };
        assert_eq!(Call::from_remote(open.clone().remote().expect("travels")), Some(open));
        assert_eq!(Call::ClearCookies(id).remote(), Some(PageCall::ClearCookies(id)));
        assert_eq!(Call::SetCookies { id, cookies: vec![] }.remote(), None);
        assert_eq!(Call::from_remote(PageCall::Refresh(id)), None, "the server answers it itself");
    }
}
