//! The `/` picker: skills then commands, filtered by what is typed, walked
//! with the arrows and paged per node.

use super::session;
use crate::state::*;

fn skill(name: &str) -> superfik_core::Skill {
    superfik_core::Skill {
        name: name.into(),
        title: String::new(),
        description: String::new(),
        icon: superfik_core::SkillIcon::Cube,
    }
}

fn labels(s: &Session) -> Vec<String> {
    s.picks().iter().map(Pick::label).collect()
}

#[test]
fn slash_lists_skills_then_commands_filtered_by_what_is_typed() {
    let mut s = session();
    s.available_skills = vec![skill("modal"), skill("dither")];
    s.apply(Action::Edit(EditKey::Insert("/mod".into())), 0.0);
    assert_eq!(s.popover, Some(PopoverKind::Skills));
    assert_eq!(labels(&s), vec!["/modal", "/model", "/mode"]);
}

#[test]
fn slash_always_opens_the_picker_because_commands_exist() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("/".into())), 0.0);
    assert_eq!(s.popover, Some(PopoverKind::Skills));
    assert_eq!(labels(&s).len(), Command::ALL.len(), "every command, paged by its node");
}

#[test]
fn an_exact_name_starts_highlighted_but_the_rows_keep_their_node_order() {
    let mut s = session();
    s.available_skills = vec![skill("memory-tips")];
    s.apply(Action::Edit(EditKey::Insert("/memory".into())), 0.0);
    assert_eq!(labels(&s), vec!["/memory-tips", "/memory"], "skills, then commands, as the nodes read");
    assert!(matches!(s.highlighted_pick(), Some(Pick::Command(Command::Memory))));
    s.apply(Action::MovePick(-1), 0.0);
    assert!(matches!(s.highlighted_pick(), Some(Pick::Skill(_))), "up goes to the row above");
}

#[test]
fn arrows_walk_the_rows_in_the_order_they_are_drawn() {
    let mut s = session();
    s.available_skills = vec![skill("built-in-browser"), skill("chrome-browser")];
    s.apply(Action::Edit(EditKey::Insert("/browser".into())), 0.0);
    assert_eq!(labels(&s), vec!["/built-in-browser", "/chrome-browser", "/browser"]);
    s.apply(Action::MovePick(-1), 0.0);
    s.apply(Action::MovePick(-1), 0.0);
    assert!(matches!(s.highlighted_pick(), Some(Pick::Skill(sk)) if sk.name == "built-in-browser"));
    s.apply(Action::MovePick(1), 0.0);
    assert!(matches!(s.highlighted_pick(), Some(Pick::Skill(sk)) if sk.name == "chrome-browser"));
}

#[test]
fn picking_a_command_runs_it_and_consumes_its_word() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("check /mem".into())), 0.0);
    let action = s.picks()[0].action();
    s.apply(action, 0.0);
    assert!(s.nodes.memory);
    assert_eq!(s.conversation().composer.text(), "check ");
    assert_eq!(s.popover, None);
}

#[test]
fn the_commands_node_pages_like_skills_and_plugins() {
    let s = session();
    assert_eq!(s.pages(Carousel::Commands), Command::ALL.len().div_ceil(5));
}

#[test]
fn the_hash_is_plain_text_now() {
    let mut s = session();
    s.apply(Action::Edit(EditKey::Insert("fix #12".into())), 0.0);
    assert_eq!(s.popover, None);
    assert_eq!(s.picking(), None);
}

fn many_skills(count: usize) -> Session {
    let mut s = session();
    s.available_skills = (0..count).map(|i| skill(&format!("skill-{i:02}"))).collect();
    s.apply(Action::Edit(EditKey::Insert("/skill".into())), 0.0);
    s
}

#[test]
fn every_skill_is_reachable_not_just_the_first_page() {
    let s = many_skills(12);
    let skills = s.picks().iter().filter(|p| matches!(p, Pick::Skill(_))).count();
    assert_eq!(skills, 12, "no cap: the picker pages instead");
    assert_eq!(s.pick_pages(Carousel::Skills), 3);
    assert_eq!(s.pick_page(Carousel::Skills), 0);
}

#[test]
fn the_arrows_turn_the_page_when_they_cross_it() {
    let mut s = many_skills(12);
    for _ in 0..5 {
        s.apply(Action::MovePick(1), 0.0);
    }
    assert_eq!(s.pick_page(Carousel::Skills), 1, "the sixth skill is on the second page");
    s.apply(Action::MovePick(-1), 0.0);
    assert_eq!(s.pick_page(Carousel::Skills), 0);
}

#[test]
fn paging_jumps_a_page_keeping_the_row_and_stops_at_the_ends() {
    let mut s = many_skills(12);
    s.apply(Action::MovePick(1), 0.0);
    s.apply(Action::PagePick(Carousel::Skills, 1), 0.0);
    assert_eq!(s.pick_cursor, 6, "same row, next page");
    s.apply(Action::PagePick(Carousel::Skills, 1), 0.0);
    assert_eq!(s.pick_cursor, 11, "the last page has only two rows: the row is kept as far as it goes");
    s.apply(Action::PagePick(Carousel::Skills, 1), 0.0);
    assert_eq!(s.pick_cursor, 11, "no page after the last");
    s.apply(Action::PagePick(Carousel::Skills, -2), 0.0);
    assert_eq!(s.pick_cursor, 1);
}

#[test]
fn paging_another_node_starts_at_its_first_row() {
    let mut s = many_skills(12);
    s.apply(Action::PagePick(Carousel::Plugins, 1), 0.0);
    assert_eq!(s.pick_cursor, 0);
}
