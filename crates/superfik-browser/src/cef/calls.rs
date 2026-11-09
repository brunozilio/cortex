//! Routes card operations to the selected native tab.
use super::*;
impl CefHost {
    pub(super) fn apply(&mut self, call: Call) {
        let id = match &call {
            Call::Open { id, .. } => *id,
            Call::Close(id)
            | Call::Navigate(id, _)
            | Call::Back(id)
            | Call::Forward(id)
            | Call::Reload(id)
            | Call::Stop(id)
            | Call::Resize { id, .. }
            | Call::Focus(id, _)
            | Call::Input(id, _)
            | Call::Agent { id, .. }
            | Call::FillLogin { id, .. }
            | Call::AnswerPasskey { id, .. }
            | Call::SetCookies { id, .. }
            | Call::ClearCookies(id) => *id,
        };
        if let Call::Open { url, profile, size, scale, .. } = call {
            self.open(id, url, &profile, size, scale);
            return;
        }
        if let Some(opening) = self.opening.get_mut(&id) {
            if matches!(call, Call::Close(_)) {
                if let Some(opening) = self.opening.remove(&id) {
                    for held in opening.held {
                        if let Call::Agent { request, .. } = held {
                            self.pending_events.push((
                                id,
                                PageEvent::Answered {
                                    request,
                                    answer: Err("The browser closed during startup.".into()),
                                },
                            ));
                        }
                    }
                }
            } else {
                opening.held.push(call);
            }
            return;
        }
        if matches!(call, Call::Close(_)) {
            self.failures.remove(&id);
            self.close_group(id);
            return;
        }
        if let Call::Agent { request, op, .. } = &call
            && self.tab_operation(id, request, op)
        {
            return;
        }
        if matches!(call, Call::Resize { .. }) {
            self.resize_tabs(id, &call);
            return;
        }
        let active = self.active_tab(id);
        if let Call::Agent { request, .. } = &call
            && let Some(error) = self.failures.get(&id)
        {
            self.pending_events
                .push((id, PageEvent::Answered { request: request.clone(), answer: Err(error.clone()) }));
        } else if let Some(page) = self.pages.get_mut(&active) {
            page.apply(call);
        } else if let Call::Agent { request, .. } = call {
            self.pending_events.push((
                id,
                PageEvent::Answered { request, answer: Err("The browser is closed or unavailable.".into()) },
            ));
        }
    }
}
