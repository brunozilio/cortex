use domain::EngineEvent;

use super::*;
use crate::transcript::Block;

fn reply(text: &str) -> Message {
    Message::assistant(vec![Block::Text(text.into())])
}

fn steered(at_ms: u64) -> Entry {
    Entry::Event { at_ms, event: EngineEvent::Steered(format!("at {at_ms}")) }
}

fn before_the_split(entry: &Entry) -> String {
    format!("{}\n", serde_json::to_string(entry).expect("json"))
}

fn said(text: &str, seq: Option<u64>) -> Line {
    Line { seq, entry: Entry::Message(Message::user(text)) }
}

fn traced(at_ms: u64, seq: Option<u64>) -> Line {
    Line { seq, entry: steered(at_ms) }
}

#[test]
fn what_was_added_is_appended_and_a_shorter_conversation_is_a_truncation() {
    let kept = [Message::user("one"), reply("two")];
    let grown = [Message::user("one"), reply("two"), Message::user("three")];
    assert_eq!(changes(&kept, &grown), vec![Entry::Message(Message::user("three"))]);
    assert_eq!(changes(&grown, &kept), vec![Entry::Truncated { keep: 2 }]);
    assert_eq!(changes(&kept, &kept), Vec::new());
}

#[test]
fn a_changed_message_rewrites_from_there_and_a_wholly_new_conversation_restarts() {
    let kept = [Message::user("one"), reply("two"), Message::user("three")];
    let mut reminded = Message::user("three");
    reminded.blocks.push(Block::Reminder("mode: plan".into()));
    let amended = [Message::user("one"), reply("two"), reminded.clone(), reply("four")];
    let entries = changes(&kept, &amended);
    assert_eq!(entries, vec![Entry::Amended { keep: 2 }, Entry::Message(reminded), Entry::Message(reply("four"))]);
    assert_eq!(conversation(kept.iter().cloned().map(Entry::Message).chain(entries)), amended);

    let summary = [Message::user("summary")];
    assert_eq!(changes(&kept, &summary), vec![Entry::Restarted, Entry::Message(Message::user("summary"))]);
}

#[test]
fn a_line_carries_its_number_and_reads_back_as_the_same_entry() {
    for entry in [Entry::Message(Message::user("hi")), Entry::Restarted, Entry::Truncated { keep: 3 }, steered(7)] {
        let line = encoded(&entry, 41).expect("line");
        assert!(line.starts_with(KIND_FIRST) && line.ends_with(",\"seq\":41}\n"), "{line}");
        assert_eq!(decoded(line.trim_end()).expect("read"), Line { seq: Some(41), entry });
    }
}

#[test]
fn an_amendment_is_a_truncation_to_a_program_that_does_not_know_the_word() {
    #[derive(Debug, PartialEq, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum BeforeAmendments {
        Truncated { keep: usize },
    }
    let line = encoded(&Entry::Amended { keep: 2 }, 9).expect("line");
    assert_eq!(line, "{\"kind\":\"truncated\",\"keep\":2,\"amended\":true,\"seq\":9}\n");
    assert_eq!(serde_json::from_str::<BeforeAmendments>(&line).expect("old"), BeforeAmendments::Truncated { keep: 2 });
    assert_eq!(decoded(line.trim_end()).expect("new").entry, Entry::Amended { keep: 2 });
    assert!(serde_json::to_string(&Entry::Amended { keep: 2 }).is_err(), "never written under a name of its own");
}

#[test]
fn a_file_from_before_the_split_reads_in_the_order_it_was_written() {
    let written = [Entry::Message(Message::user("old")), steered(1), Entry::Message(reply("answer")), steered(2)];
    let text: String = written.iter().map(before_the_split).collect();
    let read = lines(&text, Holds::Conversation).0;
    assert!(read.iter().all(|line| line.seq.is_none()));
    assert_eq!(merged(read, Vec::new()), written);
}

#[test]
fn a_file_from_before_the_split_that_grew_afterwards_keeps_its_old_part_first() {
    let old = vec![said("old", None), traced(1, None), said("older answer", None)];
    let grown = [old.clone(), vec![said("new", Some(12)), said("newer", Some(15))]].concat();
    let trace = vec![traced(10, Some(10)), traced(11, Some(11)), traced(13, Some(13)), traced(16, Some(16))];
    let order: Vec<Entry> = merged(grown, trace);
    let expected = [
        old.into_iter().map(|line| line.entry).collect::<Vec<_>>(),
        vec![steered(10), steered(11)],
        vec![Entry::Message(Message::user("new")), steered(13)],
        vec![Entry::Message(Message::user("newer")), steered(16)],
    ]
    .concat();
    assert_eq!(order, expected);
}

#[test]
fn two_files_are_read_as_one_log_in_the_order_of_their_numbers() {
    let conversation = vec![said("a", Some(3)), said("b", Some(6))];
    let trace = vec![traced(1, Some(1)), traced(2, Some(2)), traced(4, Some(4)), traced(7, Some(7))];
    let kinds: Vec<bool> = merged(conversation, trace).iter().map(Entry::is_conversation).collect();
    assert_eq!(kinds, [false, false, true, false, true, false]);
}

#[test]
fn lines_an_older_program_added_later_stay_where_they_are_in_their_file() {
    let conversation = vec![said("a", Some(5)), said("added by an older program", None), said("b", Some(9))];
    let trace = vec![traced(4, Some(4)), traced(7, Some(7))];
    let texts: Vec<String> = merged(conversation, trace)
        .into_iter()
        .map(|entry| match entry {
            Entry::Message(message) => message.text(),
            _ => "trace".to_owned(),
        })
        .collect();
    assert_eq!(texts, ["trace", "a", "added by an older program", "trace", "b"]);
}

#[test]
fn a_clock_set_back_never_reorders_a_file() {
    let conversation = vec![said("a", Some(50)), said("b", Some(20)), said("c", Some(60))];
    let texts: Vec<String> = merged(conversation, vec![traced(1, Some(30))])
        .into_iter()
        .filter_map(|entry| match entry {
            Entry::Message(message) => Some(message.text()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, ["a", "b", "c"]);
}

#[test]
fn a_trace_line_that_does_not_parse_never_stops_the_conversation_from_resuming() {
    let broken = "{\"kind\":\"called\",\"at_ms\":1,\"call\":{\"model\":\n";
    let text = [
        before_the_split(&Entry::Message(Message::user("one"))),
        broken.to_owned(),
        before_the_split(&steered(2)),
        before_the_split(&Entry::Message(reply("two"))),
    ]
    .concat();
    assert_eq!(resume(&text).history, [Message::user("one"), reply("two")]);
    let read = lines(&text, Holds::Conversation).0;
    assert_eq!(read.len(), 3, "the trace goes on without the broken line");
}

#[test]
fn a_conversation_line_that_does_not_parse_is_set_aside_and_the_rest_still_loads() {
    let broken = "{\"kind\":\"message\",\"role\":\"User\",\"blocks\":[{\"Te";
    let one = before_the_split(&Entry::Message(Message::user("one")));
    let text = [one, format!("{broken}\n"), before_the_split(&Entry::Message(reply("two")))].concat();
    let resumed = resume(&text);
    assert_eq!(resumed.history, [Message::user("one"), reply("two")]);
    assert_eq!(resumed.skipped, [broken]);
    let (read, skipped) = lines(&text, Holds::Conversation);
    assert_eq!((read.len(), skipped.as_slice()), (2, [broken.to_owned()].as_slice()));
    assert!(resume("not json at all\n{\"kind\":\"restarted\"}\n").skipped.is_empty(), "nothing before a restart");
    assert_eq!(resume("{\"kind\":\"restarted\"}\nnot json at all\n").skipped, ["not json at all"]);
}

#[test]
fn a_trace_file_is_read_around_whatever_does_not_parse() {
    let text =
        format!("{}garbage\n{{\"kind\":\"message\"\n{}", before_the_split(&steered(1)), before_the_split(&steered(2)));
    let read = lines(&text, Holds::TraceOnly).0;
    assert_eq!(read.into_iter().map(|line| line.entry).collect::<Vec<_>>(), [steered(1), steered(2)]);
}

#[test]
fn the_greatest_number_on_file_is_reported_so_new_entries_follow_it() {
    let text = [
        encoded(&Entry::Message(Message::user("one")), 70).expect("line"),
        encoded(&Entry::Message(reply("two")), 90).expect("line"),
    ]
    .concat();
    assert_eq!(resume(&text).last_seq, 90);
}
