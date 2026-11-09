// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0
//! Independent OSR surfaces for native window.open, sharing the opener context.
use super::*;
use std::collections::VecDeque;
use std::sync::{Weak, atomic::AtomicUsize};

#[derive(Default)]
pub(super) struct PopupPool {
    pub queue: Mutex<VecDeque<(i32, cef::Browser, Arc<PreparedSurface>)>>,
    pub live: AtomicUsize,
}
pub(super) struct PreparedSurface {
    pub metrics: Arc<Mutex<crate::view::ViewMetrics>>,
    pub profile_screen: Option<NativeScreen>,
    pub popup_slot: Arc<Mutex<PendingFrameSlot>>,
    pub popup: Arc<crate::popup::PopupState>,
    pub cursor: Arc<crate::cursor::LatestCursor>,
    pub ime: Arc<crate::ime::LatestComposition>,
    pub cookies: Arc<crate::cookies::CookieJar>,
    pub downloads: Arc<crate::downloads::Downloads>,
    pub auth: Arc<crate::auth::AuthChallenges>,
    pub permissions: Arc<crate::permissions::Permissions>,
    pub devtools: Arc<crate::devtools::DevToolsChannel>,
    pub snapshots: Arc<crate::snapshot::SnapshotChannel>,
    pub scripts: Arc<crate::app::PendingScripts>,
    pub frame_slot: Arc<Mutex<PendingFrameSlot>>,
    pub events: Arc<WebEventQueue>,
    pub config: MacosCefConfig,
    pub pool: Weak<PopupPool>,
    pub closed: AtomicBool,
    leased: bool,
}
impl PreparedSurface {
    pub fn client(config: &MacosCefConfig, pool: &Arc<PopupPool>, opener: Option<i32>) -> (Arc<Self>, cef::Client) {
        let initial_size = config.surface.initial_size;
        let frame_slot = Arc::new(Mutex::new(PendingFrameSlot::default()));
        let events = Arc::new(WebEventQueue::default());
        let metrics = Arc::new(Mutex::new(crate::view::ViewMetrics::new(
            initial_size,
            config
                .native_hardware
                .as_ref()
                .and_then(|v| v.screen)
                .map(|s| s.scale)
                .unwrap_or(config.surface.scale_factor),
        )));
        let popup_slot = Arc::new(Mutex::new(PendingFrameSlot::default()));
        let popup = Arc::new(crate::popup::PopupState::default());
        let cursor = Arc::new(crate::cursor::LatestCursor::default());
        let ime = Arc::new(crate::ime::LatestComposition::default());
        let cookies = Arc::new(crate::cookies::CookieJar::new(events.clone()));
        let scripts = Arc::new(crate::app::PendingScripts::new(events.clone()));
        let downloads = Arc::new(crate::downloads::Downloads::default());
        let auth = Arc::new(crate::auth::AuthChallenges::default());
        auth.set_enabled(config.surface.handle_auth_challenges);
        auth.set_proxy(config.native_hardware.as_ref().and_then(|v| v.proxy.clone()));

        let devtools = Arc::new(crate::devtools::DevToolsChannel::default());
        devtools.set_enabled(config.surface.devtools_protocol);
        let snapshots = Arc::new(crate::snapshot::SnapshotChannel::default());
        let permissions = Arc::new(crate::permissions::Permissions::default());
        permissions.set_enabled(config.surface.handle_permission_requests);
        let state = Arc::new(Self {
            metrics: metrics.clone(),
            profile_screen: config.native_hardware.as_ref().and_then(|v| v.screen),
            popup_slot: popup_slot.clone(),
            popup: popup.clone(),
            cursor: cursor.clone(),
            ime: ime.clone(),
            cookies: cookies.clone(),
            downloads: downloads.clone(),
            auth: auth.clone(),
            permissions: permissions.clone(),
            devtools: devtools.clone(),
            snapshots: snapshots.clone(),
            scripts: scripts.clone(),
            frame_slot: frame_slot.clone(),
            events: events.clone(),
            config: config.clone(),
            pool: Arc::downgrade(pool),
            closed: AtomicBool::new(false),
            leased: opener.is_some(),
        });
        let inner = WeldRenderHandlerInner {
            frame_slot: frame_slot.clone(),
            popup_slot: popup_slot.clone(),
            popup: popup.clone(),
            cursor: cursor.clone(),
            ime: ime.clone(),
            events: events.clone(),
            metrics: metrics.clone(),
            screen: config.native_hardware.as_ref().and_then(|v| v.screen),
        };
        let render_handler = cef_backed::WeldRenderHandler::build(inner.clone());
        let load_handler = cef_backed::WeldLoadHandler::build(inner.clone());
        let display_handler = cef_backed::WeldDisplayHandler::build(inner);
        let life_span_handler = cef_backed::WeldLifeSpanHandler::build(
            events.clone(),
            scripts.clone(),
            cookies.clone(),
            state.clone(),
            opener,
        );
        let request_handler = cef_backed::WeldRequestHandler::build(events.clone(), auth.clone(), scripts.clone());
        downloads.set_dir(config.surface.download_dir.clone());
        let download_handler = cef_backed::WeldDownloadHandler::build(events.clone(), downloads.clone());
        let permission_handler = cef_backed::WeldPermissionHandler::build(events.clone(), permissions.clone());
        let context_menu_handler = cef_backed::WeldContextMenuHandler::build(events.clone(), metrics.clone());
        let find_handler = cef_backed::WeldFindHandler::build(events.clone());
        let client = cef_backed::WeldClient::build(
            render_handler,
            load_handler,
            display_handler,
            life_span_handler,
            request_handler,
            download_handler,
            permission_handler,
            context_menu_handler,
            find_handler,
            config.surface.devtools_protocol,
            scripts.clone(),
            events.clone(),
        );

        (state, client)
    }
    pub fn producer(self: Arc<Self>, browser: cef::Browser, popups: Arc<PopupPool>) -> MacosCefProducer {
        MacosCefProducer {
            browser_id: browser.identifier(),
            browser,
            frame_slot: self.frame_slot.clone(),
            events: self.events.clone(),
            size: self.config.surface.initial_size,
            state: self,
            popups,
            _devtools_registration: None,
            next_snapshot_id: 1,
        }
    }
}
impl Drop for PreparedSurface {
    fn drop(&mut self) {
        if self.leased {
            if let Some(pool) = self.pool.upgrade() {
                pool.live.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }
}
impl MacosCefProducer {
    /// Newly created native popups: native opener id and a complete OSR producer.
    pub fn take_popup(&mut self) -> Option<(i32, Self)> {
        let pending = self.popups.queue.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).pop_front();
        pending.map(|(opener, browser, state)| (opener, state.producer(browser, self.popups.clone())))
    }
    pub fn is_closed(&self) -> bool {
        self.state.closed.load(Ordering::Acquire)
    }
}
