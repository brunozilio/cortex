use std::sync::Arc;
use std::time::Duration;

use domain::inbox::Notice;
use protocol::{ClientCommand, Envelope, ServerEvent};

use super::*;

#[test]
fn a_client_is_told_with_its_notices_once_that_keys_cannot_be_kept() {
    let mut services = crate::feature::testing::services(None);
    let unkept = Arc::new(crate::secrets::Unkept::because("this user has no data folder"));
    services.providers = services.providers.with_secrets(unkept);
    let (events, sent) = crossbeam_channel::unbounded();
    let handler = CommandHandler::new(services, Outbox::new(events, Arc::new(|| {})));
    let taken = || -> Vec<Notice> {
        handler.handle(Envelope::local(ClientCommand::TakeNotices));
        match sent.recv_timeout(Duration::from_secs(5)) {
            Ok(ServerEvent::Notices(notices)) => notices,
            other => panic!("expected notices, got {other:?}"),
        }
    };

    let first = taken();

    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(first[0].source, "secrets");
    assert!(
        first[0].text.contains("keys cannot be kept on this machine: this user has no data folder"),
        "{}",
        first[0].text
    );
    assert!(taken().is_empty(), "said once, not at every poll");
}
