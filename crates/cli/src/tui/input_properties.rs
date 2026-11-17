use std::panic::{AssertUnwindSafe, catch_unwind};

use ratatui::crossterm::event::KeyCode;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::super::mentions;
use super::super::pastes::Pastes;
use super::super::vim::Vim;
use super::super::wrap::{layout, wrap};
use super::Input;

const CHARS: [char; 26] = [
    'a',
    'Z',
    '7',
    '_',
    '-',
    '.',
    '/',
    '@',
    ' ',
    ' ',
    '\t',
    '\n',
    '\u{a0}',
    '\u{3000}',
    'é',
    'e',
    '\u{301}',
    '日',
    '本',
    '😀',
    '\u{1f3fd}',
    '\u{200d}',
    'ß',
    'İ',
    '[',
    '#',
];
const FILES: [&str; 4] = ["src/main.rs", "日本/文.md", "a b.txt", "x"];
const RENDERED: usize = 80;
const WIDTHS: [usize; 7] = [1, 2, 3, 4, 5, 8, 13];

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, most: usize) -> usize {
        usize::try_from(self.next() % most.max(1) as u64).unwrap_or_default()
    }

    fn char(&mut self) -> char {
        CHARS[self.below(CHARS.len())]
    }

    fn text(&mut self, most: usize) -> String {
        let length = self.below(most + 1);
        (0..length).map(|_| if self.below(12) == 0 { '\r' } else { self.char() }).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    Insert(char),
    Paste(String),
    Left,
    Right,
    Home,
    End,
    WordLeft,
    WordRight,
    Backspace,
    Delete,
    DeleteWord,
    DeleteWordForward,
    DeleteToLineStart,
    LineUp,
    LineDown,
    Older,
    Newer,
    Take,
    Clear,
    Complete(&'static str),
    EarlierHistory(Vec<String>),
    RoundTrip(usize),
    VimEscape,
    VimKey(char),
}

fn edit_op(random: &mut Random) -> Op {
    match random.below(24) {
        0..=5 => Op::Insert(random.char()),
        6 => {
            let most = if random.below(4) == 0 { 1_100 } else { 30 };
            Op::Paste(random.text(most))
        }
        7 => Op::Left,
        8 => Op::Right,
        9 => Op::Home,
        10 => Op::End,
        11 => Op::WordLeft,
        12 => Op::WordRight,
        13 => Op::Backspace,
        14 => Op::Delete,
        15 => Op::DeleteWord,
        16 => Op::DeleteWordForward,
        17 => Op::DeleteToLineStart,
        18 => [Op::LineUp, Op::LineDown][random.below(2)].clone(),
        19 => [Op::Older, Op::Newer][random.below(2)].clone(),
        20 => [Op::Take, Op::Clear][random.below(2)].clone(),
        21 => Op::Complete(FILES[random.below(FILES.len())]),
        22 => Op::EarlierHistory((0..random.below(3)).map(|_| random.text(8)).collect()),
        _ => Op::RoundTrip(1 + random.below(4)),
    }
}

fn vim_op(random: &mut Random) -> Op {
    match random.below(4) {
        0 => Op::VimEscape,
        1 => Op::VimKey("hlwb0^$jkxDdiaIAo"[random.below(17)..].chars().next().unwrap_or('h')),
        _ => edit_op(random),
    }
}

#[derive(Debug, Default)]
struct Model {
    text: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    recalled: Option<usize>,
    draft: (Vec<char>, usize),
}

fn alnum(c: char) -> bool {
    c.is_alphanumeric()
}

impl Model {
    fn string(&self) -> String {
        self.text.iter().collect()
    }

    fn byte_cursor(&self) -> usize {
        self.text[..self.cursor].iter().map(|c| c.len_utf8()).sum()
    }

    fn set(&mut self, text: &str) {
        self.text = text.chars().collect();
        self.cursor = self.text.len();
    }

    fn insert(&mut self, text: &str) {
        for c in text.chars() {
            self.text.insert(self.cursor, c);
            self.cursor += 1;
        }
        self.recalled = None;
    }

    fn line_start(&self, mut at: usize) -> usize {
        while at > 0 && self.text[at - 1] != '\n' {
            at -= 1;
        }
        at
    }

    fn line_end(&self, mut at: usize) -> usize {
        while at < self.text.len() && self.text[at] != '\n' {
            at += 1;
        }
        at
    }

    fn word_left(&self) -> usize {
        let mut at = self.cursor;
        while at > 0 && !alnum(self.text[at - 1]) {
            at -= 1;
        }
        while at > 0 && alnum(self.text[at - 1]) {
            at -= 1;
        }
        at
    }

    fn word_right(&self) -> usize {
        let mut at = self.cursor;
        while at < self.text.len() && !alnum(self.text[at]) {
            at += 1;
        }
        while at < self.text.len() && alnum(self.text[at]) {
            at += 1;
        }
        at
    }

    fn word_start(&self) -> usize {
        let mut at = self.cursor;
        while at > 0 && self.text[at - 1].is_whitespace() {
            at -= 1;
        }
        while at > 0 && !self.text[at - 1].is_whitespace() {
            at -= 1;
        }
        at
    }

    fn remove(&mut self, from: usize, to: usize) {
        self.text.drain(from..to);
        self.cursor = from;
    }

    fn line(&mut self, up: bool) {
        let start = self.line_start(self.cursor);
        let column = self.cursor - start;
        let target = if up {
            let Some(end) = start.checked_sub(1) else { return };
            self.line_start(end)
        } else {
            let end = self.line_end(self.cursor);
            if end == self.text.len() {
                return;
            }
            end + 1
        };
        self.cursor = (target + column).min(self.line_end(target));
    }

    fn browsing(&self) -> bool {
        self.recalled.and_then(|at| self.history.get(at)).is_some_and(|recalled| *recalled == self.string())
    }

    fn older(&mut self) {
        let Some(newest) = self.history.len().checked_sub(1) else { return };
        let at = match self.recalled {
            None => {
                self.draft = (self.text.clone(), self.cursor);
                newest
            }
            Some(at) => at.saturating_sub(1),
        };
        self.recalled = Some(at);
        let recalled = self.history[at].clone();
        self.set(&recalled);
    }

    fn newer(&mut self) {
        match self.recalled {
            Some(at) if at + 1 < self.history.len() => {
                self.recalled = Some(at + 1);
                let recalled = self.history[at + 1].clone();
                self.set(&recalled);
            }
            Some(_) => {
                self.recalled = None;
                (self.text, self.cursor) = std::mem::take(&mut self.draft);
            }
            None => {}
        }
    }

    fn take(&mut self) {
        let text = self.string();
        self.text.clear();
        self.cursor = 0;
        self.recalled = None;
        if !text.trim().is_empty() && self.history.last() != Some(&text) {
            self.history.push(text);
        }
    }

    fn complete(&mut self, file: &str) {
        let start = self.word_start_without_trimming();
        if self.browsing() || self.text.get(start) != Some(&'@') || start == self.cursor {
            return;
        }
        let mut rest = self.cursor;
        while self.text.get(rest) == Some(&' ') {
            rest += 1;
        }
        let word: Vec<char> = format!("@{file} ").chars().collect();
        self.text.splice(start..rest, word.iter().copied());
        self.cursor = start + word.len();
    }

    fn word_start_without_trimming(&self) -> usize {
        let mut at = self.cursor;
        while at > 0 && !self.text[at - 1].is_whitespace() {
            at -= 1;
        }
        at
    }

    fn apply(&mut self, op: &Op) {
        match op {
            Op::Insert(c) => self.insert(&c.to_string()),
            Op::Paste(_) | Op::RoundTrip(_) | Op::VimEscape | Op::VimKey(_) => {}
            Op::Left => self.cursor = self.cursor.saturating_sub(1),
            Op::Right => self.cursor = (self.cursor + 1).min(self.text.len()),
            Op::Home => self.cursor = self.line_start(self.cursor),
            Op::End => self.cursor = self.line_end(self.cursor),
            Op::WordLeft => self.cursor = self.word_left(),
            Op::WordRight => self.cursor = self.word_right(),
            Op::Backspace if self.cursor > 0 => self.remove(self.cursor - 1, self.cursor),
            Op::Delete if self.cursor < self.text.len() => self.remove(self.cursor, self.cursor + 1),
            Op::Backspace | Op::Delete => {}
            Op::DeleteWord => self.remove(self.word_start(), self.cursor),
            Op::DeleteWordForward => {
                let (start, end) = (self.cursor, self.word_right());
                self.remove(start, end);
            }
            Op::DeleteToLineStart => self.remove(self.line_start(self.cursor), self.cursor),
            Op::LineUp => self.line(true),
            Op::LineDown => self.line(false),
            Op::Older => self.older(),
            Op::Newer => self.newer(),
            Op::Take => self.take(),
            Op::Clear => {
                self.text.clear();
                self.cursor = 0;
                self.recalled = None;
            }
            Op::Complete(file) => self.complete(file),
            Op::EarlierHistory(earlier) => {
                let mut merged = earlier.clone();
                merged.append(&mut self.history);
                self.history = merged;
                self.recalled = self.recalled.map(|at| at + earlier.len());
            }
        }
    }
}

fn pasted(pastes: &mut Pastes, text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    pastes.fold(&text).unwrap_or(text)
}

fn apply(input: &mut Input, op: &Op, pastes: &mut Pastes, vim: &mut Vim) -> Result<(), String> {
    match op {
        Op::Insert(c) => input.insert(*c),
        Op::Paste(text) => input.insert_str(&pasted(pastes, text)),
        Op::Left => input.left(),
        Op::Right => input.right(),
        Op::Home => input.home(),
        Op::End => input.end(),
        Op::WordLeft => input.word_left(),
        Op::WordRight => input.word_right(),
        Op::Backspace => input.backspace(),
        Op::Delete => input.delete(),
        Op::DeleteWord => input.delete_word(),
        Op::DeleteWordForward => input.delete_word_forward(),
        Op::DeleteToLineStart => input.delete_to_line_start(),
        Op::LineUp => input.line(true),
        Op::LineDown => input.line(false),
        Op::Older => input.older(),
        Op::Newer => input.newer(),
        Op::Take => drop(input.take()),
        Op::Clear => input.clear(),
        Op::Complete(file) => mentions::complete_input(input, file),
        Op::EarlierHistory(earlier) => input.set_history(earlier.clone()),
        Op::RoundTrip(times) => return round_trip(input, *times),
        Op::VimEscape => vim.escape(input),
        Op::VimKey(c) => {
            if vim.normal {
                vim.key(KeyCode::Char(*c), input);
            }
        }
    }
    Ok(())
}

fn round_trip(input: &mut Input, times: usize) -> Result<(), String> {
    if input.recalled.is_some() {
        return Ok(());
    }
    let before = (input.text().to_owned(), input.cursor());
    for _ in 0..times {
        input.older();
    }
    for _ in 0..times {
        input.newer();
    }
    let after = (input.text().to_owned(), input.cursor());
    if after == before { Ok(()) } else { Err(format!("up {times} then down: {after:?}, the draft was {before:?}")) }
}

fn check_input(input: &Input) -> Result<(), String> {
    let (text, cursor) = (input.text(), input.cursor());
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return Err(format!("cursor {cursor} is not on a character of {text:?}"));
    }
    Ok(())
}

fn check_model(input: &Input, model: &Model) -> Result<(), String> {
    if input.text() != model.string() || input.cursor() != model.byte_cursor() {
        return Err(format!(
            "editor has {:?} at {}, the model {:?} at {}",
            input.text(),
            input.cursor(),
            model.string(),
            model.byte_cursor()
        ));
    }
    if input.history() != model.history {
        return Err(format!("history {:?}, the model {:?}", input.history(), model.history));
    }
    Ok(())
}

fn columns(text: &str) -> usize {
    text.chars().map(|c| c.width().unwrap_or(0)).sum()
}

fn fits(row_width: usize, row: &str, width: usize) -> bool {
    row_width <= width || row.chars().filter(|c| c.width().unwrap_or(0) > 0).count() == 1
}

fn check_layout(text: &str, cursor: usize, width: usize) -> Result<(), String> {
    let (rows, (row, column)) = layout(text, cursor, width);
    let context = || format!("layout({text:?}, {cursor}, {width}) = {rows:?} at {:?}", (row, column));
    if row >= rows.len() || column >= width {
        return Err(format!("the cursor is outside the rows: {}", context()));
    }
    if let Some(wide) = rows.iter().find(|shown| !fits(columns(shown), shown, width)) {
        return Err(format!("{wide:?} overflows: {}", context()));
    }
    if rows.concat() != text.replace('\n', "") {
        return Err(format!("the rows lose text: {}", context()));
    }
    let (resting, _) = layout(text, 0, width);
    let extra_row =
        cursor == text.len() && rows.len() == resting.len() + 1 && rows.last().is_some_and(String::is_empty);
    if rows[..resting.len().min(rows.len())] != resting[..] || !(rows.len() == resting.len() || extra_row) {
        return Err(format!("rows depend on the cursor, {resting:?} without it: {}", context()));
    }
    Ok(())
}

fn check_cursor_moves_forward(text: &str, width: usize) -> Result<(), String> {
    let mut last = (0, 0);
    for at in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
        let (_, here) = layout(text, at, width);
        if here < last {
            return Err(format!(
                "in {text:?} at width {width}, the cursor at {at} goes back to {here:?} from {last:?}"
            ));
        }
        last = here;
    }
    Ok(())
}

fn check_wrap(text: &str, width: usize) -> Result<(), String> {
    let lines = wrap(text, width);
    let shown = |line: &str| line.chars().filter(|c| !c.is_control()).collect::<String>().width();
    if let Some(wide) = lines.iter().find(|line| !fits(shown(line), line, width)) {
        return Err(format!("wrap({text:?}, {width}) = {lines:?}: {wide:?} overflows"));
    }
    let visible = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    if visible(&lines.concat()) != visible(text) {
        return Err(format!("wrap({text:?}, {width}) = {lines:?} loses text"));
    }
    Ok(())
}

fn against_the_model(ops: &[Op]) -> Result<(), String> {
    let (mut input, mut model, mut pastes, mut vim) =
        (Input::default(), Model::default(), Pastes::default(), Vim::default());
    let mut model_pastes = Pastes::default();
    for (step, op) in ops.iter().enumerate() {
        let fail = |error: String| format!("step {step} {op:?}: {error}");
        if let Op::Paste(text) = op {
            let inserted = pasted(&mut model_pastes, text);
            model.insert(&inserted);
        }
        apply(&mut input, op, &mut pastes, &mut vim).map_err(fail)?;
        model.apply(op);
        check_input(&input).map_err(fail)?;
        check_model(&input, &model).map_err(fail)?;
        check_layout(input.text(), input.cursor(), WIDTHS[step % WIDTHS.len()]).map_err(fail)?;
    }
    Ok(())
}

fn in_vim(ops: &[Op]) -> Result<(), String> {
    let (mut input, mut pastes, mut vim) = (Input::default(), Pastes::default(), Vim::default());
    for (step, op) in ops.iter().enumerate() {
        let fail = |error: String| format!("step {step} {op:?}: {error}");
        apply(&mut input, op, &mut pastes, &mut vim).map_err(fail)?;
        check_input(&input).map_err(fail)?;
    }
    Ok(())
}

fn rendering(ops: &[Op]) -> Result<(), String> {
    let text: String = ops
        .iter()
        .filter_map(|op| match op {
            Op::Insert(c) => Some(c.to_string()),
            Op::Paste(text) => Some(text.replace('\r', "")),
            _ => None,
        })
        .flat_map(|typed| typed.chars().collect::<Vec<_>>())
        .take(RENDERED)
        .collect();
    for width in WIDTHS {
        check_cursor_moves_forward(&text, width)?;
        check_wrap(&text, width)?;
        for at in text.char_indices().map(|(at, _)| at).chain([text.len()]) {
            check_layout(&text, at, width)?;
        }
    }
    Ok(())
}

fn pastes_unfold(ops: &[Op]) -> Result<(), String> {
    let mut pastes = Pastes::default();
    let (mut typed, mut meant) = (String::new(), String::new());
    for op in ops {
        match op {
            Op::Insert(c) if *c != '[' => {
                typed.push(*c);
                meant.push(*c);
            }
            Op::Paste(text) => {
                let text = text.replace("\r\n", "\n").replace('\r', "\n");
                typed.push_str(&pastes.fold(&text).unwrap_or_else(|| text.clone()));
                meant.push_str(&text);
            }
            _ => {}
        }
    }
    let unfolded = pastes.unfold(&typed);
    if unfolded == meant { Ok(()) } else { Err(format!("{typed:?} unfolds to {unfolded:?}, not {meant:?}")) }
}

fn guarded(property: fn(&[Op]) -> Result<(), String>, ops: &[Op]) -> Result<(), String> {
    catch_unwind(AssertUnwindSafe(|| property(ops))).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| panic.downcast_ref::<&str>().map(|text| (*text).to_owned()))
            .unwrap_or_default();
        Err(format!("panicked: {message}"))
    })
}

fn smaller(ops: &[Op]) -> Vec<Vec<Op>> {
    let mut found = Vec::new();
    for (at, op) in ops.iter().enumerate() {
        let mut fewer = ops.to_vec();
        fewer.remove(at);
        found.push(fewer);
        if let Op::Paste(text) = op {
            for (cut, c) in text.char_indices() {
                let mut shorter = ops.to_vec();
                shorter[at] = Op::Paste(format!("{}{}", &text[..cut], &text[cut + c.len_utf8()..]));
                found.push(shorter);
            }
        }
    }
    found
}

fn shrink(property: fn(&[Op]) -> Result<(), String>, mut ops: Vec<Op>) -> (Vec<Op>, String) {
    let mut error = guarded(property, &ops).err().unwrap_or_default();
    while let Some((fewer, smaller_error)) =
        smaller(&ops).into_iter().find_map(|fewer| guarded(property, &fewer).err().map(|error| (fewer, error)))
    {
        ops = fewer;
        error = smaller_error;
    }
    (ops, error)
}

fn cases(default: usize) -> usize {
    std::env::var("EDITOR_PROPERTY_CASES").ok().and_then(|cases| cases.parse().ok()).unwrap_or(default)
}

fn holds(property: fn(&[Op]) -> Result<(), String>, generate: fn(&mut Random) -> Op, cases: usize, seed: u64) {
    for case in 0..cases {
        let mut random = Random((seed ^ (case as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15)) | 1);
        let length = 1 + random.below(60);
        let ops: Vec<Op> = (0..length).map(|_| generate(&mut random)).collect();
        if guarded(property, &ops).is_err() {
            let (smallest, error) = shrink(property, ops);
            panic!("case {case} of seed {seed:#x} fails: {error}\nsmallest: {smallest:?}");
        }
    }
}

const FAST: usize = 256;
const LONG: usize = 10_000;

#[test]
fn edits_match_a_model_of_characters() {
    holds(against_the_model, edit_op, cases(FAST), 0x5eed_0001);
}

#[test]
fn vim_keys_keep_the_cursor_on_a_character() {
    holds(in_vim, vim_op, cases(FAST), 0x5eed_0002);
}

#[test]
fn typed_text_lays_out_within_the_width() {
    holds(rendering, edit_op, cases(FAST), 0x5eed_0003);
}

#[test]
fn pastes_unfold_to_what_was_pasted() {
    holds(pastes_unfold, edit_op, cases(FAST), 0x5eed_0004);
}

#[test]
#[ignore = "long run: cargo nextest run -p cli --run-ignored only editor_properties"]
fn editor_properties_long_run() {
    holds(against_the_model, edit_op, LONG, 0x10_0001);
    holds(in_vim, vim_op, LONG, 0x10_0002);
    holds(rendering, edit_op, LONG, 0x10_0003);
    holds(pastes_unfold, edit_op, LONG, 0x10_0004);
}
