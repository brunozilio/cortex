//! Native popup tabs stay inside the conversation's card and request context.
use super::{CefHost, Page, log};
use crate::{Call, PageEvent};
use serde_json::json;
use std::collections::HashMap;
use superfik_core::{BrowseId, BrowserAnswer, BrowserId, BrowserOp};
use welding::CefSurfaceProducer;

pub(super) struct Group {
    pub active: BrowserId,
    tabs: Vec<BrowserId>,
}

impl CefHost {
    pub(super) fn active_tab(&self, root: BrowserId) -> BrowserId {
        self.groups.get(&root).map_or(root, |g| g.active)
    }
    pub(super) fn owner(&self, page: BrowserId) -> BrowserId {
        self.owners.get(&page).copied().unwrap_or(page)
    }

    pub(super) fn collect_popups(&mut self) {
        let mut pending = Vec::new();
        for (id, page) in &mut self.pages {
            while let Some((opener, producer)) = page.producer.take_popup() {
                pending.push((*id, opener, producer));
            }
        }
        for (from, opener, producer) in pending {
            let parent =
                self.pages.iter().find(|(_, p)| p.producer.browser_id() == opener).map(|(id, _)| *id).unwrap_or(from);
            let root = self.owner(parent);
            let size = self.pages.get(&parent).map_or((800, 600), |p| p.size);
            let id = BrowserId::new();
            let Some(files) = self.pages.get(&parent).map(|page| page.files.clone()) else {
                continue;
            };
            self.pages.insert(id, Page::new(producer, size, files).guarded(self.egress.is_some()));
            self.owners.insert(id, root);
            self.groups.entry(root).or_insert_with(|| Group { active: root, tabs: vec![root] }).tabs.push(id);
        }
    }

    pub(super) fn close_group(&mut self, root: BrowserId) {
        let tabs = self.groups.remove(&root).map_or_else(|| vec![root], |g| g.tabs);
        for id in tabs {
            self.owners.remove(&id);
            if let Some(mut page) = self.pages.remove(&id) {
                self.pending_events.extend(
                    page.close_jobs(false)
                        .into_iter()
                        .map(|(request, answer)| (root, PageEvent::Answered { request, answer })),
                );
                while let Some((_, mut child)) = page.producer.take_popup() {
                    log(child.close());
                }
                log(page.producer.close());
            }
        }
    }

    pub(super) fn collect_closed_tabs(&mut self) {
        let closed: Vec<_> = self.pages.iter().filter(|(_, p)| p.producer.is_closed()).map(|(id, _)| *id).collect();
        for id in closed {
            let root = self.owner(id);
            if id == root {
                continue;
            }
            if let Some(mut page) = self.pages.remove(&id) {
                self.pending_events.extend(
                    page.close_jobs(true)
                        .into_iter()
                        .map(|(request, answer)| (root, PageEvent::Answered { request, answer })),
                );
            }
            self.owners.remove(&id);
            if let Some(group) = self.groups.get_mut(&root) {
                group.tabs.retain(|tab| *tab != id);
                if group.active == id {
                    group.active = root;
                }
            }
            self.report_tab(root);
        }
    }

    fn report_tab(&mut self, root: BrowserId) {
        if let Some(page) = self.pages.get(&self.active_tab(root)) {
            for event in [
                PageEvent::Address(page.url.clone()),
                PageEvent::Title(page.title.clone()),
                PageEvent::Loading(page.loading),
                PageEvent::History { back: page.history.0, forward: page.history.1 },
            ] {
                self.pending_events.push((root, event));
            }
        }
    }

    pub(super) fn tab_operation(&mut self, root: BrowserId, request: &BrowseId, op: &BrowserOp) -> bool {
        if !matches!(op, BrowserOp::Tabs | BrowserOp::SwitchTab { .. } | BrowserOp::CloseTab { .. }) {
            return false;
        }
        let result = self.tabs(root, op);
        self.pending_events.push((root, PageEvent::Answered { request: request.clone(), answer: result }));
        true
    }

    fn tabs(&mut self, root: BrowserId, op: &BrowserOp) -> Result<BrowserAnswer, String> {
        let active = self.active_tab(root);
        let ids = self.groups.get(&root).map_or_else(|| vec![root], |g| g.tabs.clone());
        if !self.pages.contains_key(&active) {
            return Err("The browser is not available.".into());
        }
        match op {
            BrowserOp::Tabs => {
                let tabs: Vec<_> = ids
                    .iter()
                    .filter_map(|id| {
                        self.pages.get(id).map(
                            |p| json!({"id":p.producer.browser_id(),"url":p.url,"title":p.title,"opener":*id==root}),
                        )
                    })
                    .collect();
                Ok(BrowserAnswer::text(
                    json!({"active":self.pages[&active].producer.browser_id(),"tabs":tabs}).to_string(),
                ))
            }
            BrowserOp::SwitchTab { tab } => {
                let target = ids
                    .into_iter()
                    .find(|id| self.pages.get(id).is_some_and(|p| p.producer.browser_id() as u32 == *tab))
                    .ok_or("This tab does not belong to this conversation.")?;
                if let Some(group) = self.groups.get_mut(&root) {
                    group.active = target;
                }
                if let Some(page) = self.pages.get_mut(&target) {
                    log(page.producer.move_focus(welding::FocusDirection::Forward));
                }
                self.report_tab(root);
                Ok(BrowserAnswer::text(format!("Selected tab {tab}.")))
            }
            BrowserOp::CloseTab { tab } => {
                let target = match tab {
                    None => active,
                    Some(tab) => ids
                        .into_iter()
                        .find(|id| self.pages.get(id).is_some_and(|p| p.producer.browser_id() as u32 == *tab))
                        .ok_or("This tab does not belong to this conversation.")?,
                };
                if target == root {
                    return Err("The conversation's opener tab stays available; close a popup tab or navigate the opener instead.".into());
                }
                if let Some(page) = self.pages.get_mut(&target) {
                    log(page.producer.close());
                }
                Ok(BrowserAnswer::text("Popup tab is closing."))
            }
            _ => Err("Not a tab operation.".into()),
        }
    }

    pub(super) fn resize_tabs(&mut self, root: BrowserId, call: &Call) {
        let ids = self.groups.get(&root).map_or_else(|| vec![root], |g| g.tabs.clone());
        for id in ids {
            if let Some(page) = self.pages.get_mut(&id) {
                page.apply(call.clone());
            }
        }
    }

    pub(super) fn routing(&self) -> HashMap<BrowserId, (BrowserId, bool)> {
        self.pages
            .keys()
            .map(|id| {
                let root = self.owner(*id);
                (*id, (root, self.active_tab(root) == *id))
            })
            .collect()
    }
}
