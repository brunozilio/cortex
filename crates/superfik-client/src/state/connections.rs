//! The `/` picker and plugin accounts in a session: accounts are named with
//! `/` like skills and commands, kept on the conversation for every prompt
//! after, removable from the Connections node.

use superfik_core::note::Note;
use superfik_core::{Connection, ConnectionId, PluginSummary, Skill};

use superfik_ui::theme::metrics;

use super::{Carousel, Command, Effect, Session, page_count};

pub const SKILL: char = '/';

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pick<'a> {
    Skill(&'a Skill),
    Connection(&'a Connection),
    Plugin(&'a PluginSummary),
    Command(Command),
    Note(&'a Note),
}

impl Pick<'_> {
    pub fn carousel(&self) -> Carousel {
        match self {
            Self::Skill(_) => Carousel::Skills,
            Self::Connection(_) | Self::Plugin(_) => Carousel::Plugins,
            Self::Command(_) => Carousel::Commands,
            Self::Note(_) => Carousel::Notes,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Skill(skill) => format!("{SKILL}{}", skill.name),
            Self::Connection(connection) => format!("{SKILL}{}", connection.slash_name()),
            Self::Plugin(plugin) => format!("{SKILL}{}", plugin.id.as_str()),
            Self::Command(command) => format!("{SKILL}{}", command.name()),
            Self::Note(note) => format!("{SKILL}{}", note.slug()),
        }
    }

    pub fn action(&self) -> super::Action {
        match self {
            Self::Skill(skill) => super::Action::CompleteSkill(skill.name.clone()),
            Self::Connection(connection) => super::Action::PickConnection(connection.id),
            Self::Plugin(plugin) => super::Action::ConnectPlugin(plugin.id.clone()),
            Self::Command(command) => super::Action::RunCommand(*command),
            Self::Note(note) => super::Action::Note(super::NoteAction::Pick(note.id)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'a> {
    pub start: usize,
    pub end: usize,
    pub pick: Pick<'a>,
}

pub fn tokens<'a>(text: &str, skills: &'a [Skill], connections: &'a [Connection], notes: &'a [Note]) -> Vec<Token<'a>> {
    let mut found = Vec::new();
    let mut offset = 0;
    for word in text.split_inclusive(char::is_whitespace) {
        let token = word.trim_end();
        let pick = token.strip_prefix(SKILL).and_then(|name| {
            let skill = skills.iter().find(|s| s.name.eq_ignore_ascii_case(name)).map(Pick::Skill);
            skill
                .or_else(|| named_connection(name, connections).map(Pick::Connection))
                .or_else(|| notes.iter().find(|n| !name.is_empty() && n.slug() == name.to_lowercase()).map(Pick::Note))
        });
        if let Some(pick) = pick {
            found.push(Token { start: offset, end: offset + token.len(), pick });
        }
        offset += word.len();
    }
    found
}

fn named_connection<'a>(name: &str, connections: &'a [Connection]) -> Option<&'a Connection> {
    if let Some(exact) = connections.iter().find(|c| c.slash_name().eq_ignore_ascii_case(name)) {
        return Some(exact);
    }
    let slug = superfik_core::plugins::slug(name);
    let mut of_plugin = connections
        .iter()
        .filter(|c| c.plugin.as_str() == slug || superfik_core::plugins::slug(&c.plugin_name) == slug);
    match (of_plugin.next(), of_plugin.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

impl Session {
    pub fn composer_tokens(&self) -> Vec<Token<'_>> {
        tokens(
            self.conversation().composer.text(),
            &self.available_skills,
            &self.plugins.connections,
            &self.notes.notes,
        )
    }

    pub fn picks(&self) -> Vec<Pick<'_>> {
        let Some((_, query)) = self.conversation().composer.trigger_query(&[SKILL]) else {
            return Vec::new();
        };
        let query = query.to_ascii_lowercase();
        let skills = self.available_skills.iter().filter(|s| s.name.to_ascii_lowercase().contains(&query));
        let used = self.session_connections();
        let accounts = self
            .plugins
            .connections
            .iter()
            .filter(|c| !used.iter().any(|u| u.id == c.id))
            .filter(|c| c.slash_name().contains(&query) || c.plugin_name.to_ascii_lowercase().contains(&query))
            .map(Pick::Connection);
        let commands = Command::ALL.iter().filter(|c| c.name().contains(&query)).copied().map(Pick::Command);
        let mut picks: Vec<Pick> = skills.map(Pick::Skill).collect();
        picks.extend(accounts.chain(self.unconnected_plugins(&query).map(Pick::Plugin)));
        picks.extend(commands);
        let linked = &self.conversation().notes;
        let notes = self.notes.notes.iter().filter(|n| !linked.contains(&n.id));
        picks.extend(notes.filter(|n| !n.slug().is_empty() && n.slug().contains(&query)).map(Pick::Note));
        picks
    }

    pub(super) fn exact_pick(&self) -> Option<usize> {
        let (_, query) = self.conversation().composer.trigger_query(&[SKILL])?;
        let exact = format!("{SKILL}{query}");
        self.picks().iter().position(|pick| pick.label().eq_ignore_ascii_case(&exact))
    }

    pub fn picking(&self) -> Option<char> {
        if self.popover != Some(super::PopoverKind::Skills) {
            return None;
        }
        self.conversation().composer.trigger_query(&[SKILL]).map(|(trigger, _)| trigger)
    }

    pub fn highlighted_pick(&self) -> Option<Pick<'_>> {
        let picks = self.picks();
        let last = picks.len().checked_sub(1)?;
        picks.get(self.pick_cursor.min(last)).copied()
    }

    fn pick_rows(&self, carousel: Carousel) -> Vec<usize> {
        let picks = self.picks();
        (0..picks.len()).filter(|i| picks[*i].carousel() == carousel).collect()
    }

    pub fn pick_pages(&self, carousel: Carousel) -> usize {
        page_count(self.pick_rows(carousel).len())
    }

    pub fn pick_page(&self, carousel: Carousel) -> usize {
        let Some(last) = self.picks().len().checked_sub(1) else { return 0 };
        let cursor = self.pick_cursor.min(last);
        self.pick_rows(carousel).iter().position(|i| *i == cursor).map_or(0, |at| at / metrics::PAGE_ROWS)
    }

    pub(super) fn page_pick(&mut self, carousel: Carousel, by: i32) -> Vec<Effect> {
        let rows = self.pick_rows(carousel);
        let Some(last) = rows.len().checked_sub(1) else { return Vec::new() };
        let cursor = self.pick_cursor.min(self.picks().len().saturating_sub(1));
        let here = rows.iter().position(|i| *i == cursor);
        let page = here.map_or(0, |at| at / metrics::PAGE_ROWS) as i64;
        let row = here.map_or(0, |at| at % metrics::PAGE_ROWS);
        let page = (page + i64::from(by)).clamp(0, (last / metrics::PAGE_ROWS) as i64) as usize;
        self.pick_cursor = rows[(page * metrics::PAGE_ROWS + row).min(last)];
        Vec::new()
    }

    pub(super) fn move_pick(&mut self, delta: i32) -> Vec<Effect> {
        let last = self.picks().len().saturating_sub(1) as i64;
        self.pick_cursor = (self.pick_cursor as i64 + i64::from(delta)).clamp(0, last) as usize;
        Vec::new()
    }

    pub fn session_connections(&self) -> Vec<&Connection> {
        let picked = self.conversation().connections.iter();
        let mut connections: Vec<&Connection> = if self.plugins.loaded {
            picked.filter(|c| self.plugins.connections.iter().any(|known| known.id == c.id)).collect()
        } else {
            picked.collect()
        };
        for token in self.composer_tokens() {
            if let Pick::Connection(c) = token.pick
                && !connections.iter().any(|known| known.id == c.id)
            {
                connections.push(c);
            }
        }
        connections
    }

    pub(super) fn pick_connection(&mut self, id: ConnectionId) -> Vec<Effect> {
        let Some(name) = self.plugins.connections.iter().find(|c| c.id == id).map(Connection::slash_name) else {
            return Vec::new();
        };
        self.popover = None;
        self.conversation_mut().composer.complete_word(&format!("{SKILL}{name}"));
        Vec::new()
    }

    pub(super) fn remove_connection(&mut self, id: ConnectionId) -> Vec<Effect> {
        let named: Vec<String> = self
            .composer_tokens()
            .iter()
            .filter(|t| matches!(t.pick, Pick::Connection(c) if c.id == id))
            .map(|t| self.conversation().composer.text()[t.start..t.end].to_owned())
            .collect();
        if self.popover == Some(super::PopoverKind::PluginAccounts) {
            self.popover = None;
        }
        let c = self.conversation_mut();
        c.connections.retain(|c| c.id != id);
        if !named.is_empty() {
            let kept: Vec<&str> =
                c.composer.text().split_whitespace().filter(|word| !named.iter().any(|n| n == word)).collect();
            c.composer = superfik_ui::TextEdit::new(kept.join(" "));
        }
        Vec::new()
    }
}
