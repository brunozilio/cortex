use ratatui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

const PASTE_START: &str = "\x1b[200~";
const PASTE_END: &str = "\x1b[201~";

pub fn decode(bytes: &[u8]) -> Vec<Event> {
    let text = String::from_utf8_lossy(bytes);
    let mut events = Vec::new();
    let mut rest: &str = &text;
    while !rest.is_empty() {
        let (event, used) = next(rest);
        events.extend(event);
        rest = &rest[used..];
    }
    events
}

fn next(text: &str) -> (Option<Event>, usize) {
    if let Some(pasted) = text.strip_prefix(PASTE_START) {
        let end = pasted.find(PASTE_END).unwrap_or(pasted.len());
        let used = PASTE_START.len() + end + if end < pasted.len() { PASTE_END.len() } else { 0 };
        return (Some(Event::Paste(pasted[..end].to_owned())), used);
    }
    if let Some(after) = text.strip_prefix('\x1b') {
        return escaped(after);
    }
    let Some(first) = text.chars().next() else { return (None, text.len()) };
    (Some(key(plain(first))), first.len_utf8())
}

fn escaped(after: &str) -> (Option<Event>, usize) {
    if let Some(csi) = after.strip_prefix('[') {
        let (code, used) = sequence(csi);
        return (code.map(key), 2 + used);
    }
    if let Some(ss3) = after.strip_prefix('O')
        && let Some(last) = ss3.chars().next()
    {
        return (final_key(last, KeyModifiers::NONE).map(key), 2 + last.len_utf8());
    }
    match after.chars().next() {
        None => (Some(key((KeyCode::Esc, KeyModifiers::NONE))), 1),
        Some(char) => {
            let (code, modifiers) = plain(char);
            (Some(key((code, modifiers | KeyModifiers::ALT))), 1 + char.len_utf8())
        }
    }
}

fn sequence(csi: &str) -> (Option<(KeyCode, KeyModifiers)>, usize) {
    let Some(end) = csi.find(|char: char| ('\x40'..='\x7e').contains(&char)) else { return (None, csi.len()) };
    let last = csi[end..].chars().next().unwrap_or('~');
    let mut numbers = csi[..end].split(';').map(|part| part.parse::<u8>().unwrap_or(1));
    let first = numbers.next().unwrap_or(1);
    let modifiers = numbers.next().map_or(KeyModifiers::NONE, modified);
    let code = match last {
        '~' => tilde(first).map(|code| (code, modifiers)),
        'Z' => Some((KeyCode::BackTab, KeyModifiers::SHIFT)),
        last => final_key(last, modifiers),
    };
    (code, end + last.len_utf8())
}

fn final_key(last: char, modifiers: KeyModifiers) -> Option<(KeyCode, KeyModifiers)> {
    let code = match last {
        'A' => KeyCode::Up,
        'B' => KeyCode::Down,
        'C' => KeyCode::Right,
        'D' => KeyCode::Left,
        'H' => KeyCode::Home,
        'F' => KeyCode::End,
        'P' => KeyCode::F(1),
        'Q' => KeyCode::F(2),
        'R' => KeyCode::F(3),
        'S' => KeyCode::F(4),
        _ => return None,
    };
    Some((code, modifiers))
}

fn tilde(number: u8) -> Option<KeyCode> {
    Some(match number {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        15 => KeyCode::F(5),
        17..=21 => KeyCode::F(number - 11),
        23 | 24 => KeyCode::F(number - 12),
        _ => return None,
    })
}

fn modified(number: u8) -> KeyModifiers {
    let bits = number.saturating_sub(1);
    let mut modifiers = KeyModifiers::NONE;
    for (bit, modifier) in [(1, KeyModifiers::SHIFT), (2, KeyModifiers::ALT), (4, KeyModifiers::CONTROL)] {
        if bits & bit != 0 {
            modifiers |= modifier;
        }
    }
    modifiers
}

fn plain(char: char) -> (KeyCode, KeyModifiers) {
    match char {
        '\r' => (KeyCode::Enter, KeyModifiers::NONE),
        '\t' => (KeyCode::Tab, KeyModifiers::NONE),
        '\x7f' => (KeyCode::Backspace, KeyModifiers::NONE),
        '\0' => (KeyCode::Char(' '), KeyModifiers::CONTROL),
        '\x01'..='\x1a' => (KeyCode::Char(char::from(b'a' - 1 + char as u8)), KeyModifiers::CONTROL),
        '\x1c'..='\x1f' => (KeyCode::Char(char::from(b'4' + (char as u8 - 0x1c))), KeyModifiers::CONTROL),
        char if char.is_uppercase() => (KeyCode::Char(char), KeyModifiers::SHIFT),
        char => (KeyCode::Char(char), KeyModifiers::NONE),
    }
}

fn key((code, modifiers): (KeyCode, KeyModifiers)) -> Event {
    Event::Key(KeyEvent::new(code, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(bytes: &str) -> Vec<(KeyCode, KeyModifiers)> {
        decode(bytes.as_bytes())
            .into_iter()
            .map(|event| match event {
                Event::Key(key) => (key.code, key.modifiers),
                other => panic!("not a key: {other:?}"),
            })
            .collect()
    }

    const NONE: KeyModifiers = KeyModifiers::NONE;

    #[test]
    fn typed_text_arrives_as_the_keys_a_terminal_would_report() {
        assert_eq!(
            keys("hé A\r"),
            [
                (KeyCode::Char('h'), NONE),
                (KeyCode::Char('é'), NONE),
                (KeyCode::Char(' '), NONE),
                (KeyCode::Char('A'), KeyModifiers::SHIFT),
                (KeyCode::Enter, NONE)
            ]
        );
        assert_eq!(keys("\t\x7f"), [(KeyCode::Tab, NONE), (KeyCode::Backspace, NONE)]);
    }

    #[test]
    fn control_keys_are_ctrl_and_a_letter_and_ctrl_j_stays_a_new_line() {
        assert_eq!(keys("\x03"), [(KeyCode::Char('c'), KeyModifiers::CONTROL)]);
        assert_eq!(keys("\n"), [(KeyCode::Char('j'), KeyModifiers::CONTROL)]);
        assert_eq!(keys("\x1f"), [(KeyCode::Char('7'), KeyModifiers::CONTROL)]);
    }

    #[test]
    fn escape_alone_is_esc_and_before_a_key_is_alt() {
        assert_eq!(keys("\x1b"), [(KeyCode::Esc, NONE)]);
        assert_eq!(keys("\x1b\r"), [(KeyCode::Enter, KeyModifiers::ALT)]);
        assert_eq!(keys("\x1bb"), [(KeyCode::Char('b'), KeyModifiers::ALT)]);
    }

    #[test]
    fn arrows_pages_and_function_keys_are_read_with_their_modifiers() {
        assert_eq!(keys("\x1b[A\x1b[D"), [(KeyCode::Up, NONE), (KeyCode::Left, NONE)]);
        assert_eq!(keys("\x1bOH\x1bOP"), [(KeyCode::Home, NONE), (KeyCode::F(1), NONE)]);
        assert_eq!(keys("\x1b[1;5C"), [(KeyCode::Right, KeyModifiers::CONTROL)]);
        assert_eq!(keys("\x1b[1;2B"), [(KeyCode::Down, KeyModifiers::SHIFT)]);
        assert_eq!(
            keys("\x1b[3~\x1b[5~\x1b[6~"),
            [(KeyCode::Delete, NONE), (KeyCode::PageUp, NONE), (KeyCode::PageDown, NONE)]
        );
        assert_eq!(keys("\x1b[15~\x1b[24~"), [(KeyCode::F(5), NONE), (KeyCode::F(12), NONE)]);
        assert_eq!(keys("\x1b[Z"), [(KeyCode::BackTab, KeyModifiers::SHIFT)]);
    }

    #[test]
    fn a_wrapped_paste_arrives_whole_and_what_follows_is_typed() {
        let events = decode(b"\x1b[200~line one\r\nline two\x1b[201~x");
        assert_eq!(
            events,
            [Event::Paste("line one\r\nline two".into()), Event::Key(KeyEvent::new(KeyCode::Char('x'), NONE))]
        );
        assert_eq!(decode(b"\x1b[200~cut"), [Event::Paste("cut".into())], "an unfinished paste is still pasted");
    }

    #[test]
    fn an_unknown_sequence_is_dropped_without_typing_its_letters() {
        assert!(keys("\x1b[99~\x1b[1;5X").is_empty());
        assert!(keys("\x1b[12").is_empty());
    }
}
