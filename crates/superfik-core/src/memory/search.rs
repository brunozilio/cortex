//! Hybrid search over memories: BM25 on words and tags, cosine similarity
//! on embedding vectors when a model is available, fused by reciprocal
//! rank and spread one hop along shared tags. The search field and prompt
//! recall share it, so what lights up is what the agent would get.

use std::collections::BTreeSet;

use super::Memory;
use super::recall::links;

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "are", "was", "but", "not", "you", "your", "our", "its",
    "into", "about", "uma", "um", "que", "para", "com", "não", "nao", "dos", "das", "nos", "nas", "por", "mais",
    "como", "ele", "ela", "isso", "esse", "essa", "de", "da", "do", "em", "no", "na", "os", "as",
];

const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;
const TAG_WEIGHT: usize = 2;
const PREFIX_MIN: usize = 3;
pub const SEMANTIC_FLOOR: f32 = 0.35;
pub const SEMANTIC_TOP: usize = 10;
const RRF_K: f32 = 10.0;
const SPREAD: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
pub struct Semantic<'a> {
    pub query: &'a [f32],
    pub memories: &'a [Option<&'a [f32]>],
}

pub fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|word| word.chars().count() >= 2)
        .map(str::to_lowercase)
        .filter(|word| !STOPWORDS.contains(&word.as_str()))
        .collect()
}

fn matches(query: &str, word: &str) -> bool {
    if query.chars().count() >= PREFIX_MIN { word.starts_with(query) } else { word == query }
}

pub fn lexical(query: &str, memories: &[Memory]) -> Vec<f32> {
    let query: BTreeSet<String> = tokens(query).into_iter().collect();
    if query.is_empty() || memories.is_empty() {
        return vec![0.0; memories.len()];
    }
    let documents: Vec<Vec<String>> = memories
        .iter()
        .map(|m| {
            let tags = m.tags.iter().flat_map(|tag| std::iter::repeat_n(tag.clone(), TAG_WEIGHT));
            tokens(&m.text).into_iter().chain(tags).collect()
        })
        .collect();
    let count = documents.len() as f32;
    let average = documents.iter().map(Vec::len).sum::<usize>() as f32 / count;
    let mut scores = vec![0.0f32; documents.len()];
    for term in &query {
        let frequencies: Vec<usize> =
            documents.iter().map(|words| words.iter().filter(|w| matches(term, w)).count()).collect();
        let with_term = frequencies.iter().filter(|f| **f > 0).count() as f32;
        if with_term == 0.0 {
            continue;
        }
        let idf = (1.0 + (count - with_term + 0.5) / (with_term + 0.5)).ln();
        for ((score, &tf), words) in scores.iter_mut().zip(&frequencies).zip(&documents) {
            let tf = tf as f32;
            let norm = BM25_K1 * (1.0 - BM25_B + BM25_B * words.len() as f32 / average.max(1.0));
            *score += idf * tf * (BM25_K1 + 1.0) / (tf + norm);
        }
    }
    scores
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    let denominator = norm(a) * norm(b);
    if denominator > 0.0 { dot / denominator } else { 0.0 }
}

pub fn similarity(semantic: Semantic) -> Vec<f32> {
    let mut scores: Vec<f32> = semantic
        .memories
        .iter()
        .map(|vector| vector.map_or(0.0, |v| cosine(semantic.query, v)))
        .map(|s| if s >= SEMANTIC_FLOOR { s } else { 0.0 })
        .collect();
    let mut order: Vec<usize> = (0..scores.len()).filter(|&i| scores[i] > 0.0).collect();
    order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]));
    for &i in order.iter().skip(SEMANTIC_TOP) {
        scores[i] = 0.0;
    }
    scores
}

fn fuse(rankings: &[Vec<f32>], len: usize) -> Vec<f32> {
    let mut fused = vec![0.0f32; len];
    for scores in rankings {
        let mut order: Vec<usize> = (0..len).filter(|&i| scores[i] > 0.0).collect();
        order.sort_by(|&a, &b| scores[b].total_cmp(&scores[a]).then(a.cmp(&b)));
        for (rank, i) in order.into_iter().enumerate() {
            fused[i] += 1.0 / (RRF_K + rank as f32 + 1.0);
        }
    }
    fused
}

pub fn search(query: &str, memories: &[Memory], semantic: Option<Semantic>) -> Vec<f32> {
    if query.trim().is_empty() {
        return vec![0.0; memories.len()];
    }
    let mut rankings = vec![lexical(query, memories)];
    if let Some(semantic) = semantic.filter(|s| s.memories.len() == memories.len()) {
        rankings.push(similarity(semantic));
    }
    let direct = fuse(&rankings, memories.len());
    let mut spread = direct.clone();
    for (a, b) in links(memories) {
        spread[a] = spread[a].max(direct[b] * SPREAD);
        spread[b] = spread[b].max(direct[a] * SPREAD);
    }
    let best = spread.iter().copied().fold(0.0f32, f32::max);
    if best > 0.0 {
        spread.iter_mut().for_each(|score| *score /= best);
    }
    spread
}

pub fn activate(query: &str, memories: &[Memory]) -> Vec<f32> {
    search(query, memories, None)
}
