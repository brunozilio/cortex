//! The Memory node and its explorer, on a project the agent has worked in
//! for a while.

use superfik_core::{Author, Memory, MemoryId, MemoryKind};

use super::scenarios::SyntheticImage;
use crate::state::{MemoryField, Session};

fn memories() -> Vec<Memory> {
    use MemoryKind::{Episodic as E, Procedural as P, Semantic as S};
    let entries: &[(MemoryKind, &str, &[&str], Option<&str>)] = &[
        (
            S,
            "The renderer draws every surface as a signed distance field, fused with a smooth-min.",
            &["render"],
            Some("Goo surfaces"),
        ),
        (
            P,
            "Run cargo clippy --workspace --all-targets -- -D warnings before committing.",
            &["ci"],
            Some("Lint setup"),
        ),
        (S, "The user writes in Portuguese; code, comments and commits stay in English.", &["user"], None),
        (
            E,
            "Added the Context node with its usage grid and the compact row.",
            &["context", "render"],
            Some("Context window"),
        ),
        (
            P,
            "Check visuals with cargo run -p superfik-app -- --snapshot /tmp/s.png --scale 2.",
            &["snapshot"],
            Some("Snapshots"),
        ),
        (S, "Every colour, size and duration comes from superfik-ui/theme.", &["theme"], Some("Design tokens")),
        (E, "Matched the composer and transcript spacing to the reference video.", &["theme"], Some("Pixel match")),
        (S, "The reducer is pure; IO happens through effects run by the shell.", &["architecture"], Some("State")),
        (P, "New strings go in text.rs, en.rs and pt_br.rs, plus the samples list.", &["i18n"], Some("Language")),
        (E, "Camera got two-finger pan and pinch zoom at the pointer.", &["camera"], Some("Canvas camera")),
        (S, "Sessions sit side by side like desktops, oldest on the left.", &["sessions"], Some("Timeline")),
        (E, "Pull request review screen landed with stacked and split diffs.", &["review"], Some("Review")),
        (
            S,
            "The Claude Code adapter resumes sessions with --resume and the init session_id.",
            &["engine"],
            Some("Engines"),
        ),
        (
            P,
            "Capture real CLI fixtures under superfik-engine/tests/fixtures before changing the parser.",
            &["engine"],
            Some("Parser"),
        ),
        (E, "Effort meter became a slider with haptic level changes.", &["haptics"], Some("Effort")),
        (S, "Haptics play through MultitouchSupport with an AppKit fallback.", &["haptics"], Some("Haptics")),
        (E, "Growing connectors carry a droplet at their tip.", &["render"], Some("Entrances")),
        (S, "Components are stateless: props in, paint, return hit geometry.", &["architecture"], Some("UI")),
        (P, "Split files over ~300 lines or functions that mix concerns.", &["architecture"], None),
        (E, "Sound packs piano and sine switch the whole app's voice.", &["sound"], Some("Sound")),
        (S, "Commands carry a Principal so a remote server can authorize them.", &["architecture"], Some("Backend")),
        (E, "Repository node lists recently opened repositories.", &["sessions"], Some("Recents")),
    ];
    entries
        .iter()
        .enumerate()
        .map(|(i, (kind, text, tags, origin))| Memory {
            id: MemoryId(i as u32),
            kind: *kind,
            text: (*text).into(),
            tags: tags.iter().map(|t| (*t).to_owned()).collect(),
            created: 1_780_000_000 + i as i64 * 3_600,
            origin: origin.map(str::to_owned),
            author: if origin.is_some() { Author::Agent } else { Author::User },
        })
        .collect()
}

pub fn memory() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = super::scenario_claude::claude();
    s.notice = None;
    s.memory.memories = memories();
    s.memory.loaded = true;
    s.memory.learned = vec![MemoryId(21)];
    s.memory.learned_at = 0.0;
    s.memory.refresh();
    (s, images)
}

pub fn memory_open() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = memory();
    s.memory.open = true;
    s.memory.focus = Some(MemoryField::Search);
    (s, images)
}

pub fn memory_search() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = memory_open();
    s.memory.query.set("render");
    let scores = superfik_core::memory::activate("render", &s.memory.memories);
    s.memory.lit = s.memory.memories.iter().zip(scores).filter(|(_, a)| *a > 0.0).map(|(m, a)| (m.id, a)).collect();
    s.memory.refresh();
    s.memory.selected = Some(MemoryId(0));
    (s, images)
}

pub fn memory_edit() -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = memory_search();
    let text = s.memory.memories[0].text.clone();
    let kind = s.memory.memories[0].kind;
    s.memory.revising = Some(crate::state::Revision { id: MemoryId(0), edit: superfik_ui::TextEdit::new(text), kind });
    s.memory.focus = Some(MemoryField::Revision);
    (s, images)
}
