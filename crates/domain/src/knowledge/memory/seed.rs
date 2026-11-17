use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::Memory;

pub const MOST_OFFERED: usize = 10;
const ALIKE: f32 = 0.6;
const FEWEST_WORDS: usize = 3;
const SHORTEST: usize = 12;
const LONGEST: usize = 240;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    pub sessions: usize,
}

pub fn candidates(sessions: &[Vec<String>], known: &[Memory]) -> Vec<Candidate> {
    let said: Vec<Vec<(String, BTreeSet<String>)>> = sessions
        .iter()
        .map(|prompts| prompts.iter().flat_map(|prompt| sentences(prompt)).map(|s| (s.clone(), words(&s))).collect())
        .collect();
    let kept: Vec<BTreeSet<String>> = known.iter().map(|memory| words(&memory.text)).collect();
    let mut offered: Vec<(Candidate, BTreeSet<String>)> = Vec::new();
    for (at, session) in said.iter().enumerate() {
        for (text, mine) in session {
            if mine.len() < FEWEST_WORDS || offered.iter().any(|(_, theirs)| alike(mine, theirs)) {
                continue;
            }
            if kept.iter().any(|theirs| alike(mine, theirs)) {
                continue;
            }
            let elsewhere = said
                .iter()
                .enumerate()
                .filter(|(other, said)| *other != at && said.iter().any(|(_, theirs)| alike(mine, theirs)))
                .count();
            if elsewhere >= 1 {
                offered.push((Candidate { text: text.clone(), sessions: elsewhere + 1 }, mine.clone()));
            }
        }
    }
    let mut offered: Vec<Candidate> = offered.into_iter().map(|(candidate, _)| candidate).collect();
    offered.sort_by_key(|candidate| std::cmp::Reverse(candidate.sessions));
    offered.truncate(MOST_OFFERED);
    offered
}

fn sentences(prompt: &str) -> Vec<String> {
    if prompt.contains("```") {
        return Vec::new();
    }
    prompt
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with(['$', '/', '!', '#', '>', '|']))
        .flat_map(|line| line.split_inclusive(['.', '?', '!', ';']))
        .map(|sentence| sentence.trim().trim_end_matches(['.', ';']).trim().to_owned())
        .filter(|sentence| (SHORTEST..=LONGEST).contains(&sentence.chars().count()) && !sentence.ends_with('?'))
        .collect()
}

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .map(str::to_lowercase)
        .filter(|word| word.chars().count() >= 3 && !COMMON.contains(&word.as_str()))
        .collect()
}

fn alike(a: &BTreeSet<String>, b: &BTreeSet<String>) -> bool {
    let shared = a.intersection(b).count();
    let all = a.union(b).count();
    shared >= FEWEST_WORDS.min(a.len()).min(b.len()) && all > 0 && shared as f32 / all as f32 >= ALIKE
}

const COMMON: &[&str] = &[
    "the", "and", "for", "that", "this", "with", "you", "are", "not", "but", "can", "have", "from", "was", "your",
    "when", "what", "will", "all", "its", "into", "then", "than", "them", "they", "our", "out", "please", "que", "com",
    "para", "uma", "por", "como", "não", "nao", "mais", "isso", "esse", "essa", "este", "esta", "dos", "das", "nos",
    "nas", "pra", "ele", "ela", "tem", "ser", "foi", "sem", "mas", "sobre",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Author, MemoryId, MemoryKind};

    fn session(prompts: &[&str]) -> Vec<String> {
        prompts.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn what_was_said_in_two_sessions_is_offered_once() {
        let sessions = [
            session(&["Fix the login bug. Always run cargo nextest instead of cargo test."]),
            session(&["always run cargo nextest, never plain cargo test", "Rename the button"]),
            session(&["Add a dark theme"]),
        ];

        let offered = candidates(&sessions, &[]);

        assert_eq!(offered, [Candidate { text: "Always run cargo nextest instead of cargo test".into(), sessions: 2 }]);
    }

    #[test]
    fn what_is_already_remembered_code_questions_and_commands_are_left_out() {
        let sessions = [
            session(&["Use pnpm for every install here", "```rust\nfn main() {}\n```", "/model sonnet please now"]),
            session(&["use pnpm for every install here", "/model sonnet please now", "Why does use pnpm fail here?"]),
        ];
        let known = Memory {
            id: MemoryId(0),
            kind: MemoryKind::Procedural,
            text: "Use pnpm for every install".into(),
            tags: Vec::new(),
            created: 0,
            origin: None,
            author: Author::User,
            history: Vec::new(),
            uses: 0,
            last_used: 0,
            touches: Vec::new(),
            credit: None,
        };

        assert_eq!(candidates(&sessions, std::slice::from_ref(&known)), []);
        assert_eq!(candidates(&sessions, &[]).len(), 1);
    }

    #[test]
    fn the_most_repeated_come_first_and_the_list_is_short() {
        let mut sessions: Vec<Vec<String>> = Vec::new();
        for n in 0..3 {
            sessions.push(session(&["Answer in Portuguese with short sentences"]));
            let many: Vec<String> =
                (0..12).map(|k| format!("Keep module number{k} {n} small and tested always")).collect();
            sessions.push(many);
        }
        sessions.push(session(&["Answer in Portuguese with short sentences"]));

        let offered = candidates(&sessions, &[]);

        assert_eq!(offered[0], Candidate { text: "Answer in Portuguese with short sentences".into(), sessions: 4 });
        assert!(offered.len() <= MOST_OFFERED);
    }
}
