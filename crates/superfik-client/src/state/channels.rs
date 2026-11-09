//! Channel accounts (ADR 0012) as the app shows them: each account's link
//! (the QR to scan while pairing, the number once linked) and the replies
//! agents drafted that wait for a member. The backend links and keeps
//! them; this keeps what it last said.

use std::collections::HashMap;

use superfik_core::ConnectionId;
use superfik_core::channel::{Draft, LinkStatus};
use superfik_protocol::ClientCommand;

use super::{Effect, Session};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelAccount {
    pub status: LinkStatus,
    pub drafts: Vec<Draft>,
}

#[derive(Debug, Clone, Default)]
pub struct ChannelsState {
    pub accounts: HashMap<ConnectionId, ChannelAccount>,
    pub editing: Option<DraftEdit>,
}

#[derive(Debug, Clone)]
pub struct DraftEdit {
    pub connection: ConnectionId,
    pub draft: u32,
    pub text: superfik_ui::TextEdit,
}

impl DraftEdit {
    pub fn is(&self, connection: ConnectionId, draft: u32) -> bool {
        self.connection == connection && self.draft == draft
    }
}

impl ChannelsState {
    pub fn drafts(&self) -> Vec<(ConnectionId, &Draft)> {
        let mut drafts: Vec<(ConnectionId, &Draft)> =
            self.accounts.iter().flat_map(|(id, account)| account.drafts.iter().map(move |d| (*id, d))).collect();
        drafts.sort_by_key(|(_, draft)| draft.written);
        drafts
    }
}

impl Session {
    pub(super) fn list_channels() -> Effect {
        Effect::Command(Box::new(ClientCommand::ListChannels))
    }

    pub(super) fn channel_arrived(&mut self, connection: ConnectionId, status: LinkStatus, drafts: Vec<Draft>) {
        self.channels.accounts.insert(connection, ChannelAccount { status, drafts });
    }

    pub(super) fn copy_channel_hook(&self, connection: ConnectionId) -> Vec<Effect> {
        match self.channels.accounts.get(&connection).map(|account| &account.status) {
            Some(LinkStatus::Linked { hook: Some(hook), .. }) => vec![Effect::CopyText(hook.clone())],
            _ => Vec::new(),
        }
    }

    fn draft(&self, connection: ConnectionId, draft: u32) -> Option<&Draft> {
        self.channels.accounts.get(&connection).and_then(|account| account.drafts.iter().find(|d| d.id == draft))
    }

    pub(super) fn edit_draft(&mut self, connection: ConnectionId, draft: u32) {
        if self.channels.editing.as_ref().is_some_and(|edit| edit.is(connection, draft)) {
            self.channels.editing = None;
            return;
        }
        let Some(text) = self.draft(connection, draft).map(|d| d.text.clone()) else { return };
        self.routines.form = None;
        self.channels.editing = Some(DraftEdit { connection, draft, text: superfik_ui::TextEdit::new(text) });
    }

    pub(super) fn approve_draft(&mut self, connection: ConnectionId, draft: u32) -> Vec<Effect> {
        let Some(written) = self.draft(connection, draft).map(|d| d.text.clone()) else { return Vec::new() };
        let edited = self
            .channels
            .editing
            .take_if(|edit| edit.is(connection, draft))
            .map(|edit| edit.text.text().trim().to_owned())
            .filter(|text| !text.is_empty());
        let text = edited.unwrap_or(written);
        vec![Effect::Command(Box::new(ClientCommand::ApproveDraft { connection, draft, text }))]
    }

    pub(super) fn discard_draft(&mut self, connection: ConnectionId, draft: u32) -> Vec<Effect> {
        self.channels.editing.take_if(|edit| edit.is(connection, draft));
        vec![Effect::Command(Box::new(ClientCommand::DiscardDraft { connection, draft }))]
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::RunId;
    use superfik_protocol::ServerEvent;

    use super::super::tests::commands;
    use super::super::{Action, RoutineAction};
    use super::*;

    fn draft(id: u32, written: i64) -> Draft {
        Draft {
            id,
            exchange: "5511999990000".into(),
            text: format!("reply {id}"),
            run: RunId::new(),
            written,
            attachment: None,
        }
    }

    fn arrived(s: &mut Session, connection: ConnectionId, status: LinkStatus, drafts: Vec<Draft>) {
        s.apply(Action::Server(ServerEvent::Channel { connection, status, drafts }), 0.0);
    }

    #[test]
    fn an_account_keeps_what_the_backend_last_said() {
        let mut s = Session::default();
        let account = ConnectionId::new();
        arrived(&mut s, account, LinkStatus::Pairing { qr: "2@abc".into() }, vec![]);
        let linked = LinkStatus::Linked { account: "5511888880000".into(), hook: None };
        arrived(&mut s, account, linked.clone(), vec![draft(0, 5)]);
        let kept = &s.channels.accounts[&account];
        assert_eq!(kept.status, linked);
        assert_eq!(kept.drafts.len(), 1);
    }

    #[test]
    fn a_linked_official_account_copies_its_webhooks_path() {
        let mut s = Session::default();
        let account = ConnectionId::new();
        let copied = |s: &mut Session| s.apply(Action::CopyChannelHook(account), 0.0);
        arrived(&mut s, account, LinkStatus::Opening, vec![]);
        assert!(!copied(&mut s).iter().any(|e| matches!(e, Effect::CopyText(_))), "not linked yet");
        let hook = format!("/hooks/channels/{account}");
        let linked = LinkStatus::Linked { account: "Superfik".into(), hook: Some(hook.clone()) };
        arrived(&mut s, account, linked, vec![]);
        assert!(copied(&mut s).iter().any(|e| matches!(e, Effect::CopyText(text) if *text == hook)));
    }

    #[test]
    fn drafts_of_every_account_wait_oldest_first() {
        let mut s = Session::default();
        let (a, b) = (ConnectionId::new(), ConnectionId::new());
        arrived(&mut s, a, LinkStatus::Opening, vec![draft(0, 30)]);
        arrived(&mut s, b, LinkStatus::Opening, vec![draft(0, 10), draft(1, 20)]);
        let order: Vec<i64> = s.channels.drafts().iter().map(|(_, d)| d.written).collect();
        assert_eq!(order, [10, 20, 30]);
    }

    #[test]
    fn approving_sends_the_draft_and_discarding_drops_it() {
        let mut s = Session::default();
        let account = ConnectionId::new();
        arrived(&mut s, account, LinkStatus::Opening, vec![draft(7, 1)]);
        let approved = s.apply(Action::Routines(RoutineAction::ApproveDraft(account, 7)), 0.0);
        assert!(matches!(
            commands(&approved)[..],
            [ClientCommand::ApproveDraft { draft: 7, text, .. }] if text == "reply 7"
        ));
        let gone = s.apply(Action::Routines(RoutineAction::ApproveDraft(account, 99)), 0.0);
        assert!(commands(&gone).is_empty());
        let discarded = s.apply(Action::Routines(RoutineAction::DiscardDraft(account, 7)), 0.0);
        assert!(matches!(commands(&discarded)[..], [ClientCommand::DiscardDraft { draft: 7, .. }]));
    }

    #[test]
    fn a_rewritten_draft_goes_out_as_rewritten() {
        let mut s = Session::default();
        let account = ConnectionId::new();
        arrived(&mut s, account, LinkStatus::Opening, vec![draft(7, 1)]);
        s.apply(Action::Routines(RoutineAction::Open), 0.0);
        s.apply(Action::Routines(RoutineAction::EditDraft(account, 7)), 0.0);
        assert_eq!(s.channels.editing.as_ref().map(|e| e.text.text().to_owned()), Some("reply 7".into()));
        s.apply(Action::Routines(RoutineAction::Edit(super::super::EditKey::Insert(", Ana".into()))), 0.0);
        let sent = s.apply(Action::Routines(RoutineAction::Submit), 0.0);
        assert!(matches!(
            commands(&sent)[..],
            [ClientCommand::ApproveDraft { draft: 7, text, .. }] if text == "reply 7, Ana"
        ));
        assert!(s.channels.editing.is_none());
    }

    #[test]
    fn escape_closes_the_draft_before_the_screen() {
        let mut s = Session::default();
        let account = ConnectionId::new();
        arrived(&mut s, account, LinkStatus::Opening, vec![draft(7, 1)]);
        s.apply(Action::Routines(RoutineAction::Open), 0.0);
        s.apply(Action::Routines(RoutineAction::EditDraft(account, 7)), 0.0);
        s.apply(Action::Routines(RoutineAction::Back), 0.0);
        assert!(s.channels.editing.is_none());
        assert!(s.routines.open);
    }

    #[test]
    fn opening_settings_or_routines_asks_for_the_accounts() {
        let mut s = Session::new(Some("/repo".into()));
        let opened = s.apply(Action::Routines(RoutineAction::Open), 0.0);
        assert!(commands(&opened).iter().any(|c| matches!(c, ClientCommand::ListChannels)));
        let settings = s.apply(Action::OpenSettings, 0.0);
        assert!(commands(&settings).iter().any(|c| matches!(c, ClientCommand::ListChannels)));
    }
}
