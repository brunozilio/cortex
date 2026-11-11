//! Animations (`tachyonfx`) drawn over what the chat already drew: the
//! conversation sweeps in at start, new parts of it and the panels fade
//! in, and "Working…" shimmers while the agent works. They change colours
//! only, never characters, and `SUPERFIK_REDUCED_MOTION=1` turns them off.

use std::time::{Duration, Instant};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use tachyonfx::pattern::SweepPattern;
use tachyonfx::{Effect, Interpolation, RefRect, fx};

/// The longest step an effect advances by, so a pause does not skip it.
const STEP: Duration = Duration::from_millis(100);
/// What faded text starts as: readable on dark and light terminals.
const FADED: Color = Color::DarkGray;

/// Where things are on this frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct Scene {
    pub conversation: Rect,
    /// How many entries the conversation has.
    pub entries: usize,
    /// The rows the entries added since the last frame take, when shown.
    pub added: Option<Rect>,
    /// The question, picker or completions above the prompt.
    pub panel: Option<Rect>,
    /// The "Working…" line, while the agent works and it is shown.
    pub working: Option<Rect>,
}

#[derive(Default)]
pub struct Motion {
    on: bool,
    effects: Vec<Effect>,
    shimmer: Option<(Effect, RefRect)>,
    last: Option<Instant>,
    started: bool,
    entries: usize,
    panel: bool,
}

impl std::fmt::Debug for Motion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Motion").field("on", &self.on).field("effects", &self.effects.len()).finish()
    }
}

impl Motion {
    pub fn new(on: bool) -> Self {
        Self { on, ..Self::default() }
    }

    pub fn from_env() -> Self {
        Self::new(std::env::var_os("SUPERFIK_REDUCED_MOTION").is_none_or(|v| v.is_empty() || v == "0"))
    }

    /// How many entries the conversation had on the last frame: those
    /// after it are new.
    pub fn seen(&self) -> usize {
        self.entries
    }

    /// Whether frames should come faster to keep an effect smooth.
    pub fn animating(&self) -> bool {
        self.on && (!self.effects.is_empty() || self.shimmer.is_some())
    }

    /// Starts the effects what changed since the last frame calls for.
    pub fn frame(&mut self, scene: Scene) {
        if !self.on {
            return;
        }
        if !self.animating() {
            self.last = None;
        }
        if !self.started {
            self.started = true;
            self.entries = scene.entries;
            let sweep = fx::fade_from_fg(FADED, (900, Interpolation::QuadOut));
            self.effects.push(sweep.with_pattern(SweepPattern::left_to_right(24)).with_area(scene.conversation));
        }
        if scene.entries > self.entries
            && let Some(rows) = scene.added
        {
            self.effects.push(fx::fade_from_fg(FADED, (350, Interpolation::QuadOut)).with_area(rows));
        }
        self.entries = scene.entries;
        if let (Some(panel), false) = (scene.panel, self.panel) {
            self.effects.push(fx::fade_from_fg(FADED, (220, Interpolation::QuadOut)).with_area(panel));
        }
        self.panel = scene.panel.is_some();
        match (scene.working, &self.shimmer) {
            (Some(row), Some((_, area))) => area.set(row),
            (Some(row), None) => {
                let area = RefRect::new(row);
                let pulse =
                    fx::lighten_fg(0.5, (700, Interpolation::SineInOut)).with_pattern(SweepPattern::left_to_right(10));
                let effect = fx::dynamic_area(area.clone(), fx::repeating(fx::ping_pong(pulse)));
                self.shimmer = Some((effect, area));
            }
            (None, _) => self.shimmer = None,
        }
    }

    /// Advances every effect by the time since the last frame and draws it.
    pub fn apply(&mut self, buffer: &mut Buffer) {
        if !self.animating() {
            return;
        }
        let now = Instant::now();
        let step = self.last.map_or(Duration::ZERO, |last| now.duration_since(last).min(STEP));
        self.last = Some(now);
        let area = buffer.area;
        for effect in &mut self.effects {
            effect.process(step, buffer, area);
        }
        self.effects.retain(Effect::running);
        if let Some((shimmer, _)) = &mut self.shimmer {
            shimmer.process(step, buffer, area);
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;

    fn scene(entries: usize) -> Scene {
        Scene { conversation: Rect::new(0, 0, 20, 5), entries, ..Scene::default() }
    }

    #[test]
    fn the_start_new_entries_and_an_opening_panel_animate_then_settle() {
        let mut motion = Motion::new(true);
        motion.frame(scene(0));
        assert!(motion.animating(), "the conversation sweeps in");
        motion.frame(Scene { added: Some(Rect::new(0, 3, 20, 2)), panel: Some(Rect::new(0, 5, 20, 1)), ..scene(1) });
        assert_eq!(motion.effects.len(), 3);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 6));
        buffer.set_string(0, 0, "hello", Style::new().fg(Color::White));
        for _ in 0..12 {
            motion.last = motion.last.map(|at| at - STEP);
            motion.apply(&mut buffer);
        }
        assert!(!motion.animating(), "every effect ended");
        assert_eq!(buffer[(0, 0)].symbol(), "h", "colours change, characters never do");
    }

    #[test]
    fn working_shimmers_until_the_agent_stops_and_reduced_motion_is_still() {
        let mut motion = Motion::new(true);
        motion.frame(scene(0));
        motion.effects.clear();
        motion.frame(Scene { working: Some(Rect::new(0, 4, 20, 1)), ..scene(0) });
        assert!(motion.shimmer.is_some());
        motion.frame(scene(0));
        assert!(!motion.animating());
        let mut still = Motion::new(false);
        still.frame(Scene { working: Some(Rect::new(0, 4, 20, 1)), ..scene(0) });
        assert!(!still.animating());
    }
}
