use std::path::Path;

use super::Readable;
use super::readers::{READERS, Reader};

pub fn expanded_by_the_shell(part: &str) -> bool {
    let bytes = part.as_bytes();
    let (mut single, mut double, mut word_starts) = (false, false, true);
    for (at, &byte) in bytes.iter().enumerate() {
        let quoted = single || double;
        match byte {
            b'\'' if !double => single = !single,
            b'"' if !single => double = !double,
            b'$' | b'`' | b'\\' if !single => return true,
            b'*' | b'?' | b'[' | b'{' | b'<' if !quoted => return true,
            b'~' if !quoted && (word_starts || !bytes.get(at + 1).is_some_and(u8::is_ascii_digit)) => return true,
            _ => {}
        }
        word_starts = !quoted && matches!(byte, b' ' | b'\t' | b'=' | b':');
    }
    false
}

pub fn with_home(part: &str, home: Option<&str>) -> String {
    let Some(home) = home.filter(|home| home.starts_with('/')) else { return part.to_owned() };
    let mut written = String::with_capacity(part.len());
    let (mut single, mut double, mut word_starts) = (false, false, true);
    let mut letters = part.chars().peekable();
    while let Some(letter) = letters.next() {
        let quoted = single || double;
        let alone = letters.peek().is_none_or(|next| matches!(next, '/' | ' ' | '\t'));
        if letter == '~' && word_starts && !quoted && alone {
            written.push_str(home.trim_end_matches('/'));
        } else {
            written.push(letter);
        }
        match letter {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ => {}
        }
        word_starts = !quoted && matches!(letter, ' ' | '\t');
    }
    written
}

pub fn only_reads(part: &str, words: &[&str], readable: Readable<'_>) -> bool {
    if expanded_by_the_shell(part) || !words.iter().all(|word| stays_inside(word, readable)) {
        return false;
    }
    let Some(reader) = reader_of(words) else { return false };
    let arguments = &words[reader.program.len()..];
    if reader.program.first() == Some(&"git") && arguments.iter().any(|word| checks_a_signature(word)) {
        return false;
    }
    arguments_allowed(reader, arguments)
}

const SIGNATURE_PLACEHOLDERS: [&str; 2] = ["%G", "%(signature"];

fn checks_a_signature(word: &str) -> bool {
    SIGNATURE_PLACEHOLDERS.iter().any(|placeholder| word.contains(placeholder))
}

pub fn is_reader(program: &str) -> bool {
    READERS.iter().any(|reader| reader.program.first() == Some(&program))
}

pub(super) fn reader_of<'a>(words: &[&str]) -> Option<&'a Reader> {
    READERS.iter().filter(|reader| words.starts_with(reader.program)).max_by_key(|reader| reader.program.len())
}

fn stays_inside(word: &str, readable: Readable<'_>) -> bool {
    let value = word.split_once('=').map_or(word, |(_, value)| value);
    [word, value].iter().all(|path| {
        !path.split('/').any(|segment| segment == "..") && (!path.starts_with('/') || readable(Path::new(path)))
    })
}

fn arguments_allowed(reader: &Reader, arguments: &[&str]) -> bool {
    let mut options_ended = false;
    arguments.iter().all(|word| {
        if options_ended || !word.starts_with('-') || *word == "-" {
            return reader.operands;
        }
        if *word == "--" {
            options_ended = true;
            return true;
        }
        flag_allowed(reader, word)
    })
}

fn flag_allowed(reader: &Reader, word: &str) -> bool {
    let listed = |flag: &str| reader.flags.contains(&flag);
    if word.starts_with("--") {
        return listed(word.split_once('=').map_or(word, |(name, _)| name));
    }
    if !reader.clustered {
        return listed(word);
    }
    for (at, letter) in word.char_indices().skip(1) {
        if letter.is_ascii_digit() {
            continue;
        }
        if listed(&format!("-{letter}=")) {
            return !word[at + letter.len_utf8()..].starts_with('/');
        }
        if !listed(&format!("-{letter}")) {
            return false;
        }
    }
    true
}
