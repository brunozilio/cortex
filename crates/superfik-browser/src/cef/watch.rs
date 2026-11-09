//! A page's DevTools stream: the capture is set up once the page answers,
//! replies go to the login fill under way, and sign-ins the user sends
//! become `PageEvent::LoginSubmitted`.

use welding::CefSurfaceProducer;

use super::Page;
use crate::PageEvent;
use crate::capture::{Submitted, capture_script};
use crate::devtools::{self, Incoming};
use crate::passkey::shim_script;

impl Page {
    pub(super) fn drain_devtools(&mut self, events: &mut Vec<PageEvent>) {
        if !self.devtools_ready {
            let setup = self.cdp.setup(&capture_script(), &shim_script(&self.passkeys.answer));
            self.devtools_ready = setup.iter().all(|message| self.producer.send_devtools_message(message).is_ok());
        }
        while let Some(message) = self.producer.poll_devtools_message() {
            match devtools::incoming(&message) {
                Some(Incoming::Reply { id, result }) => {
                    self.automation.reply(id, result.clone());
                    self.fill_reply(id, result);
                }
                Some(Incoming::Event { method, params }) => {
                    self.automation.event(&method, &params);
                    self.passkey_event(&method, &params, events);
                    let sent =
                        devtools::binding_payload(&method, &params).and_then(|payload| Submitted::read(&payload));
                    events.extend(sent.map(PageEvent::LoginSubmitted));
                }
                None => {}
            }
        }
        self.flush_automation();
    }

    pub(super) fn flush_automation(&mut self) {
        for message in std::mem::take(&mut self.automation.outbox) {
            if let Err(error) = self.producer.send_devtools_message(&message) {
                self.automation.answer = Some(Err(error.to_string()));
            }
        }
    }
}
