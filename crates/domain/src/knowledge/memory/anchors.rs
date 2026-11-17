use std::collections::HashSet;
use std::path::Path;

use super::{Memory, MemoryId};

pub const MOST_TOUCHES: usize = 8;
pub const MOST_SHOWN: usize = 4;

pub fn clean(touches: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for touch in touches {
        let Some(touch) = relative(touch) else { continue };
        if !kept.contains(&touch) && kept.len() < MOST_TOUCHES {
            kept.push(touch);
        }
    }
    kept
}

fn relative(raw: &str) -> Option<String> {
    let unix = raw.trim().replace('\\', "/");
    let mut path = unix.as_str();
    while let Some(rest) = path.strip_prefix("./") {
        path = rest;
    }
    let path = path.trim_end_matches('/');
    let outside = path.starts_with('/') || path.split('/').any(|part| part == "..") || path.contains(':');
    (!path.is_empty() && path != "." && !outside).then(|| path.to_owned())
}

pub fn widen(touches: &mut Vec<String>, more: &[String]) -> bool {
    let before = touches.len();
    for touch in more {
        if !touches.contains(touch) && touches.len() < MOST_TOUCHES {
            touches.push(touch.clone());
        }
    }
    touches.len() != before
}

pub fn relative_to(project: &Path, file: &Path) -> Option<String> {
    let inside = file.strip_prefix(project).ok()?;
    relative(&inside.to_string_lossy())
}

pub fn is_about(memory: &Memory, file: &str) -> bool {
    memory
        .touches
        .iter()
        .any(|touch| file == touch || file.strip_prefix(touch.as_str()).is_some_and(|rest| rest.starts_with('/')))
}

pub fn anchored(memories: &[Memory]) -> Vec<Memory> {
    memories.iter().filter(|memory| !memory.touches.is_empty()).cloned().collect()
}

pub fn about_file(memories: &[Memory], file: &str, shown: &mut HashSet<MemoryId>) -> Option<String> {
    let fresh: Vec<&Memory> = memories
        .iter()
        .filter(|memory| is_about(memory, file) && !shown.contains(&memory.id))
        .take(MOST_SHOWN)
        .collect();
    if fresh.is_empty() {
        return None;
    }
    shown.extend(fresh.iter().map(|memory| memory.id));
    let lines: Vec<String> = fresh.iter().map(|memory| format!("- {}", line(memory))).collect();
    Some(format!(
        "<memory>Kept from earlier sessions about {file}; follow it unless the user says otherwise, and say why \
         when you go against it:\n{}\n</memory>",
        lines.join("\n")
    ))
}

pub fn line(memory: &Memory) -> String {
    match &memory.credit {
        Some(credit) => format!("[{}] {} ({credit})", memory.id.0, memory.text),
        None => format!("[{}] {}", memory.id.0, memory.text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Author, MemoryKind};

    fn memory(id: u32, touches: &[&str]) -> Memory {
        Memory {
            id: MemoryId(id),
            kind: MemoryKind::Semantic,
            text: format!("m{id}"),
            tags: Vec::new(),
            created: 0,
            origin: None,
            author: Author::Agent,
            history: Vec::new(),
            uses: 0,
            last_used: 0,
            touches: touches.iter().map(|t| (*t).to_owned()).collect(),
            credit: None,
        }
    }

    #[test]
    fn touches_are_kept_relative_to_the_project_and_once_each() {
        let raw: Vec<String> = ["./src/a.rs", "src/a.rs", "/etc/passwd", "../x", "src\\b.rs", "crates/", " ", "C:/x"]
            .map(String::from)
            .into();
        assert_eq!(clean(&raw), ["src/a.rs", "src/b.rs", "crates"]);
        let many: Vec<String> = (0..20).map(|n| format!("f{n}.rs")).collect();
        assert_eq!(clean(&many).len(), MOST_TOUCHES);
    }

    #[test]
    fn a_memory_is_about_its_files_and_the_files_inside_its_folders() {
        let memory = memory(1, &["src/client.rs", "crates/domain"]);
        assert!(is_about(&memory, "src/client.rs"));
        assert!(is_about(&memory, "crates/domain/src/lib.rs"));
        assert!(!is_about(&memory, "crates/domain-x/lib.rs"));
        assert!(!is_about(&memory, "src/client.rs.bak"));
    }

    #[test]
    fn the_memories_about_a_file_are_shown_once_per_run() {
        let memories = [memory(1, &["a.rs"]), memory(2, &["b.rs"]), memory(3, &["a.rs"])];
        let mut shown = HashSet::new();

        let first = about_file(&memories, "a.rs", &mut shown).expect("shown");

        assert!(first.starts_with("<memory>Kept from earlier sessions about a.rs"));
        assert!(first.contains("- [1] m1\n- [3] m3"));
        assert!(!first.contains("m2"));
        assert_eq!(about_file(&memories, "a.rs", &mut shown), None);
        assert!(about_file(&memories, "b.rs", &mut shown).is_some());
    }

    #[test]
    fn a_credited_memory_says_whose_it_is() {
        let mut credited = memory(1, &["a.rs"]);
        credited.credit = Some("Maya, 2026-10-09".into());
        assert_eq!(line(&credited), "[1] m1 (Maya, 2026-10-09)");
    }

    #[test]
    fn widening_adds_only_new_touches() {
        let mut touches = vec!["a.rs".to_owned()];
        assert!(widen(&mut touches, &["a.rs".into(), "b.rs".into()]));
        assert!(!widen(&mut touches, &["b.rs".into()]));
        assert_eq!(touches, ["a.rs", "b.rs"]);
        assert_eq!(relative_to(Path::new("/p"), Path::new("/p/src/a.rs")).as_deref(), Some("src/a.rs"));
        assert_eq!(relative_to(Path::new("/p"), Path::new("/q/a.rs")), None);
        assert_eq!(anchored(&[memory(1, &[]), memory(2, &["a"])]).len(), 1);
    }
}
