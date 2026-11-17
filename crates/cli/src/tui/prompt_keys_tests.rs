use ratatui::crossterm::event::KeyCode;

use super::{Action, Armed, BINDINGS, Context, Held, Key, first_bound, resolve};

const PLAIN: Held = Held { ctrl: false, alt: false, shift: false, cmd: false };
const CTRL: Held = Held { ctrl: true, ..PLAIN };
const ALT: Held = Held { alt: true, ..PLAIN };
const SHIFT: Held = Held { shift: true, ..PLAIN };
const CMD: Held = Held { cmd: true, ..PLAIN };

const TYPED: Context = Context {
    empty: false,
    shell: false,
    busy: false,
    queued: false,
    completing: false,
    first_line: true,
    last_line: true,
    auto_retry: false,
    vim_insert: false,
    armed: Armed::No,
};
const EMPTY: Context = Context { empty: true, ..TYPED };

fn every_context() -> Vec<Context> {
    let mut all = Vec::new();
    for bits in 0u16..512 {
        let bit = |at: u16| bits & (1 << at) != 0;
        for armed in [Armed::No, Armed::Fresh, Armed::Stale] {
            all.push(Context {
                empty: bit(0),
                shell: bit(1),
                busy: bit(2),
                queued: bit(3),
                completing: bit(4),
                first_line: bit(5),
                last_line: bit(6),
                auto_retry: bit(7),
                vim_insert: bit(8),
                armed,
            });
        }
    }
    all
}

fn every_held() -> Vec<Held> {
    (0u8..16)
        .map(|bits| Held { ctrl: bits & 1 != 0, alt: bits & 2 != 0, shift: bits & 4 != 0, cmd: bits & 8 != 0 })
        .collect()
}

fn check(cases: &[(&str, Context, KeyCode, Held, Option<Action>)]) {
    for (name, context, code, held, expected) in cases {
        assert_eq!(resolve(context, *code, *held), *expected, "{name}: {code:?} {held:?} in {context:?}");
    }
}

#[test]
fn esc_resolves_by_the_first_of_its_eight_rows_that_holds() {
    let esc = KeyCode::Esc;
    check(&[
        (
            "vim insert mode comes first",
            Context { vim_insert: true, busy: true, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::VimEscape),
        ),
        (
            "vim insert mode before shell mode",
            Context { vim_insert: true, shell: true, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::VimEscape),
        ),
        (
            "an empty shell prompt leaves shell mode",
            Context { shell: true, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::LeaveShellMode),
        ),
        (
            "even while a run goes on",
            Context { shell: true, busy: true, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::LeaveShellMode),
        ),
        ("a run is interrupted", Context { busy: true, ..EMPTY }, esc, PLAIN, Some(Action::Interrupt)),
        ("a run is interrupted with text typed", Context { busy: true, ..TYPED }, esc, PLAIN, Some(Action::Interrupt)),
        (
            "a run is interrupted before a retry or an armed clear",
            Context { busy: true, auto_retry: true, armed: Armed::Fresh, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::Interrupt),
        ),
        (
            "a pending retry is cancelled",
            Context { auto_retry: true, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::CancelAutoRetry),
        ),
        (
            "a pending retry is cancelled before rewind opens",
            Context { auto_retry: true, armed: Armed::Fresh, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::CancelAutoRetry),
        ),
        (
            "a pending retry stays while text is typed",
            Context { auto_retry: true, ..TYPED },
            esc,
            PLAIN,
            Some(Action::ArmClear),
        ),
        (
            "the second esc on an empty prompt opens rewind",
            Context { armed: Armed::Fresh, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::OpenRewind),
        ),
        (
            "too late: an empty prompt is armed again",
            Context { armed: Armed::Stale, ..EMPTY },
            esc,
            PLAIN,
            Some(Action::ArmClear),
        ),
        ("the first esc on an empty prompt arms", EMPTY, esc, PLAIN, Some(Action::ArmClear)),
        (
            "the second esc clears a typed prompt",
            Context { armed: Armed::Fresh, ..TYPED },
            esc,
            PLAIN,
            Some(Action::ClearArmedPrompt),
        ),
        ("however late", Context { armed: Armed::Stale, ..TYPED }, esc, PLAIN, Some(Action::ClearArmedPrompt)),
        ("the first esc on a typed prompt arms", TYPED, esc, PLAIN, Some(Action::ArmClear)),
        ("a typed shell command is armed too", Context { shell: true, ..TYPED }, esc, PLAIN, Some(Action::ArmClear)),
        ("modifiers do not matter", Context { busy: true, ..TYPED }, esc, CTRL, Some(Action::Interrupt)),
    ]);
}

#[test]
fn left_resolves_by_the_first_of_its_four_rows_that_holds() {
    let left = KeyCode::Left;
    check(&[
        ("an empty prompt opens the fleet", EMPTY, left, PLAIN, Some(Action::OpenFleet)),
        ("shift does not stop it", EMPTY, left, SHIFT, Some(Action::OpenFleet)),
        ("while a run goes on too", Context { busy: true, ..EMPTY }, left, PLAIN, Some(Action::OpenFleet)),
        ("not in shell mode", Context { shell: true, ..EMPTY }, left, PLAIN, Some(Action::Left)),
        ("not with text typed", TYPED, left, PLAIN, Some(Action::Left)),
        ("cmd goes to the line start, empty or not", EMPTY, left, CMD, Some(Action::LineStart)),
        ("cmd goes to the line start", TYPED, left, CMD, Some(Action::LineStart)),
        ("cmd wins over alt", TYPED, left, Held { alt: true, ..CMD }, Some(Action::LineStart)),
        ("alt moves a word, empty or not", EMPTY, left, ALT, Some(Action::WordLeft)),
        ("alt moves a word", TYPED, left, ALT, Some(Action::WordLeft)),
        ("ctrl moves a word", TYPED, left, CTRL, Some(Action::WordLeft)),
        ("plain moves a character", TYPED, left, SHIFT, Some(Action::Left)),
    ]);
}

#[test]
fn the_other_keys_resolve_in_each_context_that_changes_them() {
    let completing = Context { completing: true, ..TYPED };
    let queued = Context { busy: true, queued: true, ..EMPTY };
    let inside = Context { first_line: false, last_line: false, ..TYPED };
    let shell = Context { shell: true, ..EMPTY };
    let char = KeyCode::Char;
    check(&[
        ("right", TYPED, KeyCode::Right, PLAIN, Some(Action::Right)),
        ("cmd right", TYPED, KeyCode::Right, CMD, Some(Action::LineEnd)),
        ("alt right", TYPED, KeyCode::Right, ALT, Some(Action::WordRight)),
        ("ctrl right", TYPED, KeyCode::Right, CTRL, Some(Action::WordRight)),
        ("backspace leaves an empty shell prompt", shell, KeyCode::Backspace, PLAIN, Some(Action::LeaveShellMode)),
        ("backspace leaves it whatever is held", shell, KeyCode::Backspace, CMD, Some(Action::LeaveShellMode)),
        ("backspace", TYPED, KeyCode::Backspace, PLAIN, Some(Action::Backspace)),
        ("backspace on an empty prompt", EMPTY, KeyCode::Backspace, PLAIN, Some(Action::Backspace)),
        ("cmd backspace", TYPED, KeyCode::Backspace, CMD, Some(Action::DeleteToLineStart)),
        ("cmd wins over alt", TYPED, KeyCode::Backspace, Held { alt: true, ..CMD }, Some(Action::DeleteToLineStart)),
        ("alt backspace", TYPED, KeyCode::Backspace, ALT, Some(Action::DeleteWord)),
        ("delete", TYPED, KeyCode::Delete, PLAIN, Some(Action::Delete)),
        ("alt delete", TYPED, KeyCode::Delete, ALT, Some(Action::DeleteWordForward)),
        ("ctrl delete", TYPED, KeyCode::Delete, CTRL, Some(Action::DeleteWordForward)),
        ("up walks the suggestions", completing, KeyCode::Up, PLAIN, Some(Action::SuggestionUp)),
        ("down walks the suggestions", completing, KeyCode::Down, PLAIN, Some(Action::SuggestionDown)),
        (
            "suggestions before a queued message",
            Context { completing: true, ..queued },
            KeyCode::Up,
            PLAIN,
            Some(Action::SuggestionUp),
        ),
        ("up takes back what is queued", queued, KeyCode::Up, PLAIN, Some(Action::TakeBackQueued)),
        ("not with text typed", Context { empty: false, ..queued }, KeyCode::Up, PLAIN, Some(Action::OlderPrompt)),
        ("not once the run ended", Context { busy: false, ..queued }, KeyCode::Up, PLAIN, Some(Action::OlderPrompt)),
        ("up recalls from the first line", TYPED, KeyCode::Up, PLAIN, Some(Action::OlderPrompt)),
        ("down recalls from the last line", TYPED, KeyCode::Down, PLAIN, Some(Action::NewerPrompt)),
        ("up moves a line inside the text", inside, KeyCode::Up, PLAIN, Some(Action::LineUp)),
        ("down moves a line inside the text", inside, KeyCode::Down, PLAIN, Some(Action::LineDown)),
        ("enter sends", TYPED, KeyCode::Enter, PLAIN, Some(Action::Submit)),
        ("enter sends while busy", Context { busy: true, ..TYPED }, KeyCode::Enter, PLAIN, Some(Action::Submit)),
        ("alt enter", TYPED, KeyCode::Enter, ALT, Some(Action::NewLine)),
        ("shift enter", TYPED, KeyCode::Enter, SHIFT, Some(Action::NewLine)),
        ("ctrl j", TYPED, char('j'), CTRL, Some(Action::NewLine)),
        ("tab", TYPED, KeyCode::Tab, PLAIN, Some(Action::Complete)),
        ("shift tab", TYPED, KeyCode::BackTab, SHIFT, Some(Action::NextMode)),
        ("home", TYPED, KeyCode::Home, PLAIN, Some(Action::LineStart)),
        ("end", TYPED, KeyCode::End, PLAIN, Some(Action::LineEnd)),
        ("page up", TYPED, KeyCode::PageUp, PLAIN, Some(Action::PageUp)),
        ("page down", TYPED, KeyCode::PageDown, PLAIN, Some(Action::PageDown)),
        ("a bang on an empty prompt", EMPTY, char('!'), PLAIN, Some(Action::EnterShellMode)),
        ("alt does not stop it", EMPTY, char('!'), ALT, Some(Action::EnterShellMode)),
        ("ctrl does", EMPTY, char('!'), CTRL, None),
        ("a bang in shell mode is typed", shell, char('!'), PLAIN, Some(Action::Insert('!'))),
        ("a bang after text is typed", TYPED, char('!'), PLAIN, Some(Action::Insert('!'))),
        ("alt b", TYPED, char('b'), ALT, Some(Action::WordLeft)),
        ("alt f", TYPED, char('f'), ALT, Some(Action::WordRight)),
        ("alt d", TYPED, char('d'), ALT, Some(Action::DeleteWordForward)),
        ("alt d before ctrl d", EMPTY, char('d'), Held { alt: true, ..CTRL }, Some(Action::DeleteWordForward)),
        ("ctrl d quits an empty prompt", EMPTY, char('d'), CTRL, Some(Action::Quit)),
        ("ctrl d does nothing otherwise", TYPED, char('d'), CTRL, None),
        ("ctrl r", TYPED, char('r'), CTRL, Some(Action::SearchHistory)),
        ("ctrl g", TYPED, char('g'), CTRL, Some(Action::EditExternally)),
        ("ctrl l", TYPED, char('l'), CTRL, Some(Action::Redraw)),
        ("ctrl a", TYPED, char('a'), CTRL, Some(Action::LineStart)),
        ("ctrl e", TYPED, char('e'), CTRL, Some(Action::LineEnd)),
        ("ctrl u", TYPED, char('u'), CTRL, Some(Action::EmptyThePrompt)),
        ("ctrl v", TYPED, char('v'), CTRL, Some(Action::PasteImage)),
        ("ctrl w", TYPED, char('w'), CTRL, Some(Action::DeleteWord)),
        ("a letter is typed", TYPED, char('d'), PLAIN, Some(Action::Insert('d'))),
        ("with shift", TYPED, char('D'), SHIFT, Some(Action::Insert('D'))),
        ("with alt when alt means nothing for it", TYPED, char('x'), ALT, Some(Action::Insert('x'))),
        ("not with ctrl", TYPED, char('x'), CTRL, None),
        ("a key nothing is bound to", TYPED, KeyCode::F(5), PLAIN, None),
    ]);
}

#[test]
fn every_binding_is_the_first_match_somewhere() {
    let (contexts, helds) = (every_context(), every_held());
    for (at, binding) in BINDINGS.iter().enumerate() {
        let code = match binding.key {
            Key::Code(code) => code,
            Key::AnyChar => KeyCode::Char('z'),
        };
        let reached =
            contexts.iter().any(|context| helds.iter().any(|held| first_bound(context, code, *held) == Some(at)));
        assert!(reached, "row {at} is shadowed by the rows above it: {binding:?}");
    }
}

#[test]
fn the_rows_of_one_key_sit_together_in_priority_order() {
    let mut seen: Vec<Key> = Vec::new();
    for binding in BINDINGS {
        if seen.last() == Some(&binding.key) {
            continue;
        }
        assert!(!seen.contains(&binding.key), "{:?} has rows in two places", binding.key);
        seen.push(binding.key);
    }
    assert_eq!(seen.last(), Some(&Key::AnyChar), "typing a character is the last resort");
}

#[test]
fn esc_and_left_have_the_rows_the_prompt_is_known_for() {
    let rows = |code: KeyCode| BINDINGS.iter().filter(|binding| binding.key == Key::Code(code)).count();
    assert_eq!(rows(KeyCode::Esc), 8);
    assert_eq!(rows(KeyCode::Left), 4);
}
