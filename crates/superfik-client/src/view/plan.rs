//! Decides which nodes exist for the current session and lays them out.

use superfik_core::ToolCall;
use superfik_i18n::Text;

use super::layout::{CanvasLayout, NodeKey, layout, place_browser};
use super::nodes::{agent, browser, context, memory, notes, pull_request, references, repository, research, skills};
use crate::state::Session;

pub fn context_nodes(session: &Session) -> Vec<(NodeKey, f32)> {
    let c = session.conversation();
    let mut nodes = vec![(NodeKey::Repository, repository::height(&repository_props(session)))];
    let picking = session.picking().is_some();
    if picking {
        nodes.push((NodeKey::Skills, skills::picker_height(picking_rows(session, NodeKey::Skills))));
    } else if session.nodes.skills {
        nodes.push((NodeKey::Skills, skills::height(session.selected_skills().len())));
    }
    let rows = session.plugin_rows().len();
    if picking {
        nodes.push((NodeKey::Connections, skills::picker_height(picking_rows(session, NodeKey::Connections))));
    } else if rows > 0 && session.nodes.plugins {
        nodes.push((NodeKey::Connections, super::nodes::connections::height(rows)));
    }
    if picking {
        nodes.push((NodeKey::Commands, skills::picker_height(picking_rows(session, NodeKey::Commands))));
    } else if session.nodes.commands {
        nodes.push((NodeKey::Commands, super::nodes::commands::height()));
    }
    if !c.references.is_empty() {
        nodes.push((NodeKey::References, references::height(c.references.len())));
    }
    match picking_rows(session, NodeKey::Notes) {
        0 => {
            if let Some(height) = with_notes_props(session, false, notes::height) {
                nodes.push((NodeKey::Notes, height));
            }
        }
        picks => nodes.push((NodeKey::Notes, skills::picker_height(picks))),
    }
    nodes
}

pub fn with_notes_props<R>(
    session: &Session,
    caret_visible: bool,
    f: impl FnOnce(&notes::NotesProps) -> R,
) -> Option<R> {
    let state = &session.notes;
    let ids = &session.conversation().notes;
    let linked: Vec<_> = ids.iter().filter_map(|id| state.get(*id)).collect();
    let loose: Vec<_> =
        if session.nodes.notes { state.notes.iter().filter(|n| !ids.contains(&n.id)).collect() } else { Vec::new() };
    let editing = state.editing.as_ref().map(|d| (d.id, &d.edit, d.paper));
    let shows = session.root.is_some() && (!linked.is_empty() || session.nodes.notes || editing.is_some());
    shows.then(|| f(&notes::NotesProps { linked: &linked, loose: &loose, editing, caret_visible }))
}

pub fn output_nodes(session: &Session) -> Vec<(NodeKey, f32)> {
    let c = session.conversation();
    let mut nodes = Vec::new();
    if session.root.is_some() && session.nodes.memory {
        nodes.push((NodeKey::Memory, memory::height()));
    }
    if let Some(props) = context_props(session) {
        nodes.push((NodeKey::Context, context::height(&props)));
    }
    nodes.extend((0..c.agents.len()).map(|i| (NodeKey::Agent(i), agent::height())));
    nodes
}

pub fn memory_props(session: &Session, caret_visible: bool, now: f64) -> memory::MemoryProps<'_> {
    memory::MemoryProps { state: &session.memory, caret_visible, now }
}

pub fn picking(session: &Session, key: NodeKey) -> Vec<(usize, crate::state::Pick<'_>)> {
    use crate::state::Pick;
    if session.picking().is_none() {
        return Vec::new();
    }
    let fills = |pick: &Pick| {
        matches!(
            (key, pick),
            (NodeKey::Skills, Pick::Skill(_))
                | (NodeKey::Connections, Pick::Connection(_) | Pick::Plugin(_))
                | (NodeKey::Commands, Pick::Command(_))
                | (NodeKey::Notes, Pick::Note(_))
        )
    };
    session.picks().into_iter().enumerate().filter(|(_, pick)| fills(pick)).collect()
}

fn picking_rows(session: &Session, key: NodeKey) -> usize {
    picking(session, key).len()
}

pub fn carousel_of(key: NodeKey) -> crate::state::Carousel {
    use crate::state::Carousel;
    match key {
        NodeKey::Connections => Carousel::Plugins,
        NodeKey::Commands => Carousel::Commands,
        NodeKey::Notes => Carousel::Notes,
        _ => Carousel::Skills,
    }
}

pub fn focused_nodes(session: &Session) -> Vec<NodeKey> {
    [NodeKey::Skills, NodeKey::Connections, NodeKey::Commands, NodeKey::Notes]
        .into_iter()
        .filter(|key| session.picking().is_some() && (*key != NodeKey::Notes || !picking(session, *key).is_empty()))
        .collect()
}

pub fn plan(session: &Session, area: superfik_render::Rect) -> CanvasLayout {
    let keys = focused_nodes(session);
    if !keys.is_empty() {
        let focused: Vec<(NodeKey, f32)> =
            context_nodes(session).into_iter().filter(|(k, _)| keys.contains(k)).collect();
        return layout(area, &focused, &[], None, None);
    }
    let c = session.conversation();
    let research = c.shows_research().then(research::height);
    let beside = c.pull_request.as_ref().map(|_| pull_request::height());
    let mut layout = layout(area, &context_nodes(session), &output_nodes(session), research, beside);
    if c.browser.is_some() {
        place_browser(&mut layout, browser::height());
    }
    layout
}

pub fn context_props(session: &Session) -> Option<context::ContextProps<'_>> {
    let c = session.conversation();
    session.nodes.context.then(|| context::ContextProps {
        usage: c.context.as_ref(),
        measuring: c.is_measuring(),
        compacting: c.is_compacting(),
        can_compact: c.can_compact(),
        compaction: c.compaction,
    })
}

pub fn repository_props(session: &Session) -> repository::RepositoryProps<'_> {
    repository::RepositoryProps {
        repository: session.snapshot.repository.as_ref(),
        branch: session.branch.as_deref().unwrap_or("main"),
        pull_requests: session.pull_requests.len(),
        attached: &session.conversation().repositories,
        activity: None,
    }
}

pub fn repository_activity(session: &Session) -> Option<String> {
    let turn = session.conversation().turns.last()?;
    let reads: Vec<&ToolCall> =
        turn.response.tools.iter().filter(|t| matches!(t.name.as_str(), "Read" | "Glob" | "Grep")).collect();
    if session.is_gathering() {
        let path = |t: &ToolCall| relative(&t.summary, session.root.as_deref());
        return reads.last().map(|t| session.strings().t(Text::ReadingFile(&path(t))));
    }
    (!reads.is_empty()).then(|| session.strings().t(Text::ReadFiles(reads.len())))
}

fn relative(path: &str, root: Option<&std::path::Path>) -> String {
    let root = root.map(|r| r.to_string_lossy().trim_end_matches('/').to_owned());
    match root.as_deref().and_then(|r| path.strip_prefix(r)) {
        Some(rest) => rest.trim_start_matches('/').to_owned(),
        None => path.to_owned(),
    }
}

#[cfg(test)]
mod tests;
