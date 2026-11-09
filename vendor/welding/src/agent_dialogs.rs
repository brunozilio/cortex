// Copyright 2026 Superfik contributors.
// SPDX-License-Identifier: MPL-2.0
//! Keep Linux OSR dialogs alive for an explicit DevTools answer. CEF's
//! default Linux dialog runner requires a native parent window, which an
//! off-screen surface does not have. Chromium's Page domain owns the actual
//! answer; its HandleJavaScriptDialog then closes this placeholder handler.

use cef::*;
use std::sync::{Arc, Mutex};

cef::wrap_jsdialog_handler! {
    struct AgentDialogs {
        pending: Arc<Mutex<Option<cef::JsdialogCallback>>>,
    }

    impl JsdialogHandler {
        fn on_jsdialog(
            &self,
            _browser: Option<&mut Browser>,
            _origin_url: Option<&CefString>,
            _dialog_type: JsdialogType,
            _message_text: Option<&CefString>,
            _default_prompt_text: Option<&CefString>,
            callback: Option<&mut JsdialogCallback>,
            _suppress_message: Option<&mut std::os::raw::c_int>,
        ) -> std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            let mut pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if pending.is_some() {
                return 0;
            }
            *pending = Some(callback.clone());
            1
        }

        fn on_dialog_closed(&self, _browser: Option<&mut Browser>) {
            self.clear();
        }

        fn on_reset_dialog_state(&self, _browser: Option<&mut Browser>) {
            self.clear();
        }
    }
}

impl AgentDialogs {
    fn clear(&self) {
        let pending = self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner).take();
        // Dropping the callback can re-enter CEF's dialog manager.
        drop(pending);
    }
}

pub(crate) fn handler() -> cef::JsdialogHandler {
    AgentDialogs::new(Arc::default())
}
