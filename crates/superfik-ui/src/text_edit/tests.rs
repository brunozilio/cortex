use super::*;

#[test]
fn inserts_and_deletes_at_caret() {
    let mut edit = TextEdit::new("hey");
    edit.move_left(false);
    edit.insert("!");
    assert_eq!(edit.text(), "he!y");
    edit.backspace();
    edit.delete();
    assert_eq!(edit.text(), "he");
}

#[test]
fn handles_multibyte_chars() {
    let mut edit = TextEdit::new("café");
    edit.backspace();
    assert_eq!(edit.text(), "caf");
    edit.insert("é✨");
    edit.move_left(false);
    edit.backspace();
    assert_eq!(edit.text(), "caf✨");
}

#[test]
fn selection_replaces_and_deletes() {
    let mut edit = TextEdit::new("hello world");
    edit.select_all();
    assert_eq!(edit.selected_text(), Some("hello world"));
    edit.insert("hi");
    assert_eq!(edit.text(), "hi");
    edit.move_left(true);
    edit.backspace();
    assert_eq!(edit.text(), "h");
}

#[test]
fn backspace_word() {
    let mut edit = TextEdit::new("make it move");
    edit.backspace_word();
    assert_eq!(edit.text(), "make it ");
}

#[test]
fn trigger_query_and_completion() {
    let mut edit = TextEdit::new("/dither-minis /mo");
    assert_eq!(edit.trigger_query(&['/', '#']), Some(('/', "mo")));
    assert_eq!(edit.trigger_query(&['#']), None);
    edit.complete_word("/motion");
    assert_eq!(edit.text(), "/dither-minis /motion ");
    assert_eq!(edit.trigger_query(&['/']), None);
    let edit = TextEdit::new("ask #git");
    assert_eq!(edit.trigger_query(&['/', '#']), Some(('#', "git")));
}

#[test]
fn clearing_the_word_at_the_caret() {
    let mut edit = TextEdit::new("send it #gma");
    edit.clear_word();
    assert_eq!(edit.text(), "send it ");
    let mut spaced = TextEdit::new("done ");
    spaced.clear_word();
    assert_eq!(spaced.text(), "done ", "nothing typed since the space");
}

#[test]
fn a_double_click_selects_the_word_and_copy_gets_it() {
    let mut edit = TextEdit::new("fix /motion now");
    edit.select_word_at(7);
    assert_eq!(edit.selected_text(), Some("/motion"));
    edit.select_word_at(3);
    assert_eq!(edit.selected_text(), Some(" "));
    edit.select_word_at(15);
    assert_eq!(edit.selected_text(), Some("now"), "the end of the text takes the word before");
    edit.select_word_at(0);
    assert_eq!(edit.selected_text(), Some("fix"));
}

#[test]
fn moving_to_an_offset_selects_when_asked() {
    let mut edit = TextEdit::new("café au lait");
    edit.move_to(2, false);
    edit.move_to(9, true);
    assert_eq!(edit.selected_text(), Some("fé au "));
    edit.move_to(4, false);
    assert_eq!((edit.caret(), edit.selection()), (3, None), "inside é: back to its start");
}

#[test]
fn take_empties_field() {
    let mut edit = TextEdit::new("send me");
    assert_eq!(edit.take(), "send me");
    assert!(edit.is_empty());
    assert_eq!(edit.caret(), 0);
}
