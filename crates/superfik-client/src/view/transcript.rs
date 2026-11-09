//! The conversation column above the composer, anchored to its bottom.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use superfik_core::days::clock_time;
use superfik_core::{Activity, FailureKind, Turn, TurnId, conversation::format_seconds};
use superfik_i18n::{Strings, Text};
use superfik_render::{ImageId, Point, Rect};
use superfik_ui::Painter;
use superfik_ui::components::{Button, Reply, StatusLine, UserBubble};
use superfik_ui::theme::{metrics, motion, typography};

use super::LoadedImage;
use super::motions::{Motions, WordsScope};
use super::steps;
use super::target::Target;

mod reply;

use reply::{as_reply, blocks, drawn};

const RETRY_GAP: f32 = 12.0;
const RETRY_ROW: f32 = metrics::CHIP_HEIGHT;
const RETRY_LABEL_GAP: f32 = 10.0;

pub fn failure_message(strings: &Strings, failure: Option<FailureKind>, utc_offset: i32) -> Option<String> {
    match failure? {
        FailureKind::UsageLimit { resets_at: Some(at) } => {
            Some(strings.t(Text::UsageLimitResets(&clock_time(at, utc_offset))))
        }
        FailureKind::UsageLimit { resets_at: None } => Some(strings.t(Text::UsageLimitReached)),
        FailureKind::Unavailable => Some(strings.t(Text::ServiceUnavailable)),
        FailureKind::Other => None,
    }
}

fn offers_retry(props: &TranscriptProps, turn: &Turn) -> bool {
    props.retry && matches!(turn.response.activity, Activity::Failed(_))
}

const TURN_GAP: f32 = 35.0;
const STATUS_GAP: f32 = 22.0;
const REPLY_GAP: f32 = 22.0;

#[derive(Debug)]
pub struct TranscriptProps<'a> {
    pub turns: &'a [Turn],
    pub live_since: Option<f64>,
    pub images: &'a HashMap<PathBuf, LoadedImage>,
    pub x: f32,
    pub bottom: f32,
    pub fly_from: Option<Point>,
    pub words: WordsScope,
    pub now: f64,
    pub root: Option<&'a std::path::Path>,
    pub steps_open: &'a [Option<f64>],
    pub strings: Strings,
    pub retry: bool,
    pub hovered: Option<Target>,
    pub utc_offset: i32,
    pub open_outputs: &'a HashSet<(TurnId, usize)>,
    pub comments: &'a [superfik_core::comments::Comment],
    pub sends: bool,
}

impl TranscriptProps<'_> {
    fn steps_open(&self, index: usize) -> Option<f64> {
        self.steps_open.get(index).copied().flatten()
    }

    fn comments_after(&self, index: usize) -> impl Iterator<Item = &superfik_core::comments::Comment> {
        let last = index + 1 == self.turns.len();
        self.comments.iter().filter(move |c| c.after_turn == index || (last && c.after_turn > index))
    }
}

fn comment_bubble<'a>(comment: &'a superfik_core::comments::Comment, caption: &'a str) -> UserBubble<'a> {
    UserBubble { text: &comment.text, images: &[], caption, max_width: 320.0 }
}

fn comment_caption(strings: &Strings, comment: &superfik_core::comments::Comment) -> String {
    strings.t(Text::CommentBy(&comment.author_name))
}

fn bubble<'a>(turn: &'a Turn, thumbs: &'a [ImageId]) -> UserBubble<'a> {
    UserBubble { text: &turn.user.text, images: thumbs, caption: &turn.user.branch_caption, max_width: 372.0 }
}

fn thumbs(turn: &Turn, images: &HashMap<PathBuf, LoadedImage>) -> Vec<ImageId> {
    turn.user.images.iter().filter_map(|p| images.get(p).map(|i| i.id)).collect()
}

fn status(strings: &Strings, turn: &Turn, live_since: Option<f64>, now: f64) -> (String, String) {
    let elapsed = live_since.map_or(turn.response.elapsed_ms, |s| ((now - s) * 1000.0) as u64);
    let label = strings.activity(&turn.response.activity, elapsed);
    let detail = if live_since.is_some() {
        format_seconds(elapsed)
    } else if turn.response.activity == Activity::Done {
        strings.t(Text::Steps(turn.response.steps()))
    } else {
        String::new()
    };
    (label, detail)
}

pub fn height(p: &mut Painter, props: &TranscriptProps) -> f32 {
    let width = metrics::TRANSCRIPT_WIDTH - 3.0;
    let mut h = 0.0;
    for (i, turn) in props.turns.iter().enumerate() {
        if i > 0 {
            h += TURN_GAP;
        }
        let ids = thumbs(turn, props.images);
        h += bubble(turn, &ids).height(p) + STATUS_GAP + StatusLine::HEIGHT;
        h += steps::height(turn, props.steps_open(i), props.open_outputs);
        if offers_retry(props, turn) {
            h += RETRY_GAP + RETRY_ROW;
        }
        let blocks = blocks(turn);
        if !blocks.is_empty() {
            let reply = as_reply(&blocks, &[]);
            h += REPLY_GAP + Reply { blocks: &reply, width, reveal: &|_| 1.0 }.height(p);
        }
        for comment in props.comments_after(i) {
            let caption = comment_caption(&props.strings, comment);
            h += REPLY_GAP + comment_bubble(comment, &caption).height(p);
        }
    }
    h
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn retry_row(p: &mut Painter, props: &TranscriptProps, turn: &Turn, index: usize, row: Rect) -> Rect {
    let cy = row.center().y;
    let mut x = row.x;
    if let Some(message) = failure_message(&props.strings, turn.response.failure, props.utc_offset) {
        let label = p.label(&message, typography::STATUS, x, cy, p.palette().text_secondary);
        x += label.width + RETRY_LABEL_GAP;
    }
    let text = props.strings.t(Text::RetryTurn);
    let button = Button {
        label: &text,
        primary: false,
        hovered: props.hovered == Some(Target::RetryTurn(index)),
        enabled: true,
    };
    let rect = Rect::new(x, row.y, button.width(p), RETRY_ROW);
    button.paint(p, rect);
    rect
}

pub fn paint(
    p: &mut Painter,
    props: &TranscriptProps,
    motions: &mut Motions,
    shift: f32,
    clickable: &mut Vec<(Target, Rect)>,
) -> Vec<Rect> {
    let mut status_rects = Vec::with_capacity(props.turns.len());
    let width = metrics::TRANSCRIPT_WIDTH - 3.0;
    let mut y = props.bottom - height(p, props) + shift;
    let right = props.x + width;
    let last = props.turns.len().saturating_sub(1);
    for (index, turn) in props.turns.iter().enumerate() {
        if index > 0 {
            y += TURN_GAP;
        }
        let ids = thumbs(turn, props.images);
        let user = bubble(turn, &ids);
        let bubble_height = user.height(p);
        let fly = props.live_since.filter(|_| index == last).zip(props.fly_from).map(|(since, from)| {
            let t = ((props.now - since) as f32 / motion::SEND_FLY).clamp(0.0, 1.0);
            (from, motion::MORPH_EASE.apply(t))
        });
        match fly.filter(|(_, t)| *t < 1.0) {
            Some((from, t)) => {
                let start_right = from.x + user.width(p);
                let start_top = from.y - user.thumbs_height();
                user.paint(p, lerp(start_right, right, t), lerp(start_top, y, t));
            }
            None => user.paint(p, right, y),
        }
        y += bubble_height + STATUS_GAP;

        let live_since = props.live_since.filter(|_| index == last);
        let (label, detail) = status(&props.strings, turn, live_since, props.now);
        let live = live_since.is_some();
        let open = props.steps_open(index);
        let line = StatusLine { label: &label, detail: &detail, live, time: props.now as f32, open: open.is_some() };
        status_rects.push(line.paint(p, Point::new(props.x, y)));
        y += StatusLine::HEIGHT;
        y += steps::paint(p, props, turn, index, open, Point::new(props.x, y), width, clickable);
        if offers_retry(props, turn) {
            let row = Rect::new(props.x, y + RETRY_GAP, width, RETRY_ROW);
            clickable.push((Target::RetryTurn(index), retry_row(p, props, turn, index, row)));
            y += RETRY_GAP + RETRY_ROW;
        }

        let blocks = blocks(turn);
        if !blocks.is_empty() {
            y += REPLY_GAP;
            let words = turn.response.text.split_whitespace().count();
            motions.sync_words(props.words, index, words, props.now);
            let drawn = drawn(&blocks);
            let reply = as_reply(&blocks, &drawn);
            let reveal = |word: usize| motions.word(props.words, index, word, props.now);
            let mut visuals = Vec::new();
            let rect = Reply { blocks: &reply, width, reveal: &reveal }.paint(p, Point::new(props.x, y), &mut visuals);
            if props.sends {
                super::visual_hits::visual_prompts(&blocks, &visuals, index, clickable);
            }
            y += rect.h;
        }
        for comment in props.comments_after(index) {
            let caption = comment_caption(&props.strings, comment);
            let bubble = comment_bubble(comment, &caption);
            y += REPLY_GAP;
            bubble.paint(p, right, y);
            y += bubble.height(p);
        }
    }
    status_rects
}

#[cfg(test)]
mod tests;
