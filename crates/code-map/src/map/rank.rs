use std::collections::{HashMap, HashSet};

use super::words::{PromptTerms, prompt_terms, significant, split_identifier, stem};
use super::{CodeMap, Pointer, SymbolHit};
use crate::extract::{FileSymbols, Symbol};

pub(crate) const SYMBOLS_PER_FILE: usize = 4;
pub(crate) const CALLER_FILES: usize = 3;
const EXACT_IDENTIFIER: u32 = 20;
const EXACT_PATH: u32 = 24;
const WHOLE_WORD: u32 = 6;
const PATH_WORD: u32 = 3;

struct Scored<'map> {
    file: &'map FileSymbols,
    score: u32,
    hits: Vec<(&'map Symbol, u32)>,
}

pub(crate) fn relevant(map: &CodeMap, prompt: &str, limit: usize) -> Vec<Pointer> {
    let terms = prompt_terms(prompt);
    if terms.words.is_empty() && terms.identifiers.is_empty() && terms.paths.is_empty() {
        return Vec::new();
    }
    let defined_in = definitions(map);
    let mut scored: Vec<Scored> = map.files.iter().filter_map(|file| score_file(file, &terms, &defined_in)).collect();
    let best = scored.iter().map(|entry| entry.score).max().unwrap_or(0);
    scored.retain(|entry| entry.score * 4 >= best);
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.file.path.cmp(&b.file.path)));
    scored.truncate(limit);
    let wanted: HashSet<&str> =
        scored.iter().flat_map(|entry| entry.hits.iter().map(|(symbol, _)| symbol.name.as_str())).collect();
    let calls = callers_by_name(map, &wanted);
    scored.into_iter().map(|entry| pointer(entry, &calls)).collect()
}

fn score_file<'map>(
    file: &'map FileSymbols,
    terms: &PromptTerms,
    defined_in: &HashMap<&str, usize>,
) -> Option<Scored<'map>> {
    let mut hits: Vec<(&Symbol, u32)> = file
        .symbols
        .iter()
        .map(|symbol| {
            let spread = defined_in.get(symbol.name.as_str()).copied().unwrap_or(1).max(1);
            (symbol, score_symbol(&symbol.name, terms, spread))
        })
        .filter(|(_, score)| *score > 0)
        .collect();
    hits.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.line.cmp(&b.0.line)));
    hits.dedup_by(|a, b| a.0.name == b.0.name);
    hits.truncate(SYMBOLS_PER_FILE);
    let score = hits.iter().map(|(_, score)| score).sum::<u32>() + score_path(&file.path, terms);
    (score > 0).then_some(Scored { file, score, hits })
}

fn score_symbol(name: &str, terms: &PromptTerms, spread: usize) -> u32 {
    if terms.identifiers.iter().any(|identifier| identifier.eq_ignore_ascii_case(name)) {
        return EXACT_IDENTIFIER;
    }
    let spread = u32::try_from(spread).unwrap_or(u32::MAX);
    if terms.plain.contains(&stem(&name.to_lowercase())) {
        return WHOLE_WORD / spread;
    }
    let parts = significant(split_identifier(name));
    let matched = u32::try_from(parts.intersection(&terms.words).count()).unwrap_or(u32::MAX);
    if matched >= 2 && usize::try_from(matched).is_ok_and(|count| count == parts.len()) {
        return matched * 4 / spread;
    }
    if matched >= 2 { matched * 2 / spread } else { matched / spread }
}

fn score_path(path: &str, terms: &PromptTerms) -> u32 {
    if terms.paths.iter().any(|mentioned| path == mentioned || path.ends_with(&format!("/{mentioned}"))) {
        return EXACT_PATH;
    }
    let stem = path.rsplit_once('.').map_or(path, |(stem, _)| stem);
    let words = significant(stem.split('/').flat_map(split_identifier));
    u32::try_from(words.intersection(&terms.words).count()).unwrap_or(u32::MAX) * PATH_WORD
}

fn definitions(map: &CodeMap) -> HashMap<&str, usize> {
    let mut defined_in: HashMap<&str, usize> = HashMap::new();
    for file in &map.files {
        let names: HashSet<&str> = file.symbols.iter().map(|symbol| symbol.name.as_str()).collect();
        for name in names {
            *defined_in.entry(name).or_default() += 1;
        }
    }
    defined_in
}

struct Callers<'map> {
    count: usize,
    files: Vec<&'map str>,
}

fn callers_by_name<'map>(map: &'map CodeMap, wanted: &HashSet<&str>) -> HashMap<String, Callers<'map>> {
    let mut found: HashMap<String, Callers> = HashMap::new();
    for file in &map.files {
        for call in &file.calls {
            if !wanted.contains(call.callee.as_str()) {
                continue;
            }
            let entry = found.entry(call.callee.clone()).or_insert(Callers { count: 0, files: Vec::new() });
            entry.count += 1;
            if !entry.files.contains(&file.path.as_str()) {
                entry.files.push(&file.path);
            }
        }
    }
    found
}

fn pointer(entry: Scored, calls: &HashMap<String, Callers>) -> Pointer {
    let path = entry.file.path.clone();
    let symbols = entry
        .hits
        .into_iter()
        .map(|(symbol, _)| {
            let callers = calls.get(&symbol.name);
            let mut files: Vec<&str> = callers.map(|found| found.files.clone()).unwrap_or_default();
            files.sort_by_key(|file| *file == path);
            SymbolHit {
                name: symbol.name.clone(),
                kind: symbol.kind,
                line: symbol.line,
                callers: callers.map_or(0, |found| found.count),
                caller_files: files.into_iter().take(CALLER_FILES + 1).map(str::to_owned).collect(),
            }
        })
        .collect();
    Pointer { path, symbols }
}
