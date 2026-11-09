//! The session timeline: every conversation of the workspace, newest
//! first, grouped by the local day it started.

use superfik_core::{ConversationId, DayGroup, days::local_day};
use superfik_i18n::Text;

use super::{Effect, Session};

pub const SHORTCUTS: usize = 9;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineItem {
    pub id: ConversationId,
    pub title: String,
    pub running: bool,
    pub waiting: bool,
    pub selected: bool,
    pub attention: Option<Attention>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attention {
    Asking,
    Failed,
    Done,
}

pub fn attention_of(conversation: &super::Conversation) -> Option<Attention> {
    if conversation.is_waiting() {
        return Some(Attention::Asking);
    }
    if !conversation.ended_unseen || conversation.is_running() {
        return None;
    }
    match conversation.turns.last()?.response.activity {
        superfik_core::Activity::Failed(_) => Some(Attention::Failed),
        superfik_core::Activity::Done => Some(Attention::Done),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineGroup {
    pub label: String,
    pub day: i64,
    pub collapsed: bool,
    pub items: Vec<TimelineItem>,
    pub archived: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toward {
    Older,
    Newer,
}

impl Session {
    pub fn timeline(&self, now: f64) -> Vec<TimelineGroup> {
        let now_unix = self.clock.wall_now(now);
        let offset = self.clock.utc_offset_secs;
        let started = self.started();
        let group = |unix: i64, items: Vec<TimelineItem>| {
            let day = local_day(unix, offset);
            TimelineGroup {
                label: self.strings().day(DayGroup::of(unix, now_unix, offset), now_unix, offset),
                day,
                collapsed: self.collapsed_days.contains(&day),
                items,
                archived: false,
            }
        };
        let mut groups: Vec<TimelineGroup> = Vec::new();
        if let Some(fresh) = self.conversations.iter().find(|c| c.id == self.active && c.started_unix.is_none()) {
            groups.push(group(now_unix, vec![self.item(fresh)]));
        }
        for conversation in started {
            let unix = conversation.started_unix.unwrap_or(now_unix);
            let item = self.item(conversation);
            match groups.last_mut() {
                Some(last) if last.day == local_day(unix, offset) => last.items.push(item),
                _ => groups.push(group(unix, vec![item])),
            }
        }
        groups.extend(self.shared_group());
        let archived: Vec<TimelineItem> = self.archived().into_iter().map(|c| self.item(c)).collect();
        if !archived.is_empty() {
            groups.push(TimelineGroup {
                label: self.strings().t(Text::ArchivedSessions),
                day: i64::MIN,
                collapsed: !self.archived_open,
                items: archived,
                archived: true,
            });
        }
        self.mark_presence(&mut groups);
        groups
    }

    pub fn neighbour(&self, toward: Toward) -> Option<ConversationId> {
        let (order, at) = self.stepping_order()?;
        let len = order.len();
        let next = match toward {
            Toward::Older => (at + 1) % len,
            Toward::Newer => (at + len - 1) % len,
        };
        Some(order[next]).filter(|id| *id != self.active)
    }

    pub fn neighbour_within(&self, toward: Toward) -> Option<ConversationId> {
        let (order, at) = self.stepping_order()?;
        let next = match toward {
            Toward::Older => at + 1,
            Toward::Newer => at.checked_sub(1)?,
        };
        order.get(next).copied()
    }

    fn stepping_order(&self) -> Option<(Vec<ConversationId>, usize)> {
        let fresh = self.conversations.iter().filter(|c| c.id == self.active && c.started_unix.is_none());
        let order: Vec<ConversationId> = fresh.chain(self.started()).map(|c| c.id).collect();
        let at = order.iter().position(|id| *id == self.active)?;
        Some((order, at))
    }

    pub fn shortcut_order(&self, now: f64) -> Vec<ConversationId> {
        self.timeline(now)
            .iter()
            .filter(|g| !g.archived && !g.collapsed)
            .flat_map(|g| g.items.iter().map(|i| i.id))
            .take(SHORTCUTS)
            .collect()
    }

    pub(super) fn select_shortcut(&mut self, digit: usize, now: f64) -> Vec<Effect> {
        match self.shortcut_order(now).get(digit.wrapping_sub(1)).copied() {
            Some(id) => self.select_conversation(id),
            None => Vec::new(),
        }
    }

    pub(super) fn step_conversation(&mut self, toward: Toward) -> Vec<Effect> {
        match self.neighbour(toward) {
            Some(id) => self.select_conversation(id),
            None => Vec::new(),
        }
    }

    pub(super) fn scroll_conversation(&mut self, toward: Toward) -> Vec<Effect> {
        match self.neighbour_within(toward) {
            Some(id) => self.select_conversation(id),
            None => Vec::new(),
        }
    }

    fn started(&self) -> Vec<&super::Conversation> {
        let mut started: Vec<&super::Conversation> = self
            .conversations
            .iter()
            .filter(|c| c.started_unix.is_some() && !c.archived && c.shared_by.is_none())
            .collect();
        started.sort_by_key(|c| std::cmp::Reverse(c.started_unix));
        started
    }

    fn archived(&self) -> Vec<&super::Conversation> {
        let mut archived: Vec<&super::Conversation> =
            self.conversations.iter().filter(|c| c.started_unix.is_some() && c.archived).collect();
        archived.sort_by_key(|c| std::cmp::Reverse(c.started_unix));
        archived
    }

    pub(super) fn toggle_archived(&mut self) -> Vec<Effect> {
        self.archived_open = !self.archived_open;
        Vec::new()
    }

    pub(super) fn toggle_group(&mut self, day: i64) -> Vec<Effect> {
        if !self.collapsed_days.remove(&day) {
            self.collapsed_days.insert(day);
        }
        Vec::new()
    }

    fn item(&self, conversation: &super::Conversation) -> TimelineItem {
        TimelineItem {
            id: conversation.id,
            title: conversation.title.clone().unwrap_or_else(|| self.strings().t(Text::NewSession)),
            running: conversation.is_running(),
            waiting: conversation.is_waiting(),
            selected: conversation.id == self.active,
            attention: attention_of(conversation),
        }
    }
}
