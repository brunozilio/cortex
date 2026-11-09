//! The Context node, measured after a reply and after a compaction.

use superfik_core::{Compaction, ContextShare, ContextSlice, ContextUsage, SessionId};

use super::scenarios::SyntheticImage;
use crate::state::Session;

pub fn context() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = super::scenario_claude::claude();
    s.notice = None;
    let c = s.conversation_mut();
    c.engine_session = Some(SessionId("scenario".into()));
    let share = |slice, tokens| ContextShare { slice, tokens };
    c.context = Some(ContextUsage {
        used: 164_800,
        window: 1_000_000,
        reserved: 33_000,
        shares: vec![
            share(ContextSlice::SystemPrompt, 4_200),
            share(ContextSlice::Tools, 20_300),
            share(ContextSlice::Mcp, 2_400),
            share(ContextSlice::Memory, 435),
            share(ContextSlice::Skills, 6_000),
            share(ContextSlice::Messages, 131_500),
        ],
    });
    (s, images)
}

pub fn compacted() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = context();
    let c = s.conversation_mut();
    c.compaction = Some(Compaction { before: 164_800, after: 38_200 });
    if let Some(usage) = &mut c.context {
        usage.used = 38_200;
        if let Some(messages) = usage.shares.last_mut() {
            messages.tokens = 4_900;
        }
    }
    (s, images)
}
