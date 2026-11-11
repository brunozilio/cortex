use super::*;

fn memory(id: u32) -> Memory {
    Memory {
        id: MemoryId(id),
        kind: MemoryKind::Semantic,
        text: format!("Fact {id}"),
        tags: Vec::new(),
        created: 0,
        origin: None,
        author: Author::Agent,
        history: Vec::new(),
        uses: 0,
        last_used: 0,
    }
}

fn ids(answer: &SearchAnswer) -> Vec<u32> {
    match answer {
        SearchAnswer::Found(found) => found.iter().map(|m| m.id.0).collect(),
        other => panic!("expected memories, got {other:?}"),
    }
}

#[test]
fn memory_already_in_the_prompt_is_not_found_again() {
    let mut searches = MemorySearches::new([MemoryId(1)]);
    let answer = searches.answer(vec![memory(1), memory(2), memory(3)], 1);
    assert_eq!(ids(&answer), vec![2], "the limit counts only what is new");
}

#[test]
fn what_one_search_returned_the_next_one_skips() {
    let mut searches = MemorySearches::new([]);
    assert_eq!(ids(&searches.answer(vec![memory(4), memory(5)], 8)), vec![4, 5]);
    assert_eq!(searches.answer(vec![memory(5)], 8), SearchAnswer::AlreadyShown);
}

#[test]
fn nothing_matching_is_told_apart_from_nothing_new() {
    let mut searches = MemorySearches::new([MemoryId(7)]);
    assert_eq!(searches.answer(Vec::new(), 8), SearchAnswer::Nothing);
    assert_eq!(searches.answer(vec![memory(7)], 8), SearchAnswer::AlreadyShown);
}

#[test]
fn a_run_searches_a_bounded_number_of_times() {
    let mut searches = MemorySearches::new([]);
    for _ in 0..MemorySearches::LIMIT {
        assert_eq!(searches.answer(Vec::new(), 8), SearchAnswer::Nothing);
    }
    assert_eq!(searches.answer(vec![memory(1)], 8), SearchAnswer::Exhausted);
}
