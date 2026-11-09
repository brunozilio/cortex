//! When each part of Get Started moves: pure functions of the seconds
//! since it opened (`theme::motion::GET_STARTED_*`), each `0..=1`.

use crate::theme::motion as m;

fn span(elapsed: f32, start: f32, duration: f32) -> f32 {
    m::ENTER.apply(linear(elapsed, start, duration))
}

fn linear(elapsed: f32, start: f32, duration: f32) -> f32 {
    ((elapsed - start) / duration.max(f32::EPSILON)).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beat(pub f32);

impl Beat {
    pub fn darken(self) -> f32 {
        span(self.0, 0.0, m::GET_STARTED_DARKEN)
    }

    pub fn word(self, i: usize) -> f32 {
        span(self.0, m::GET_STARTED_WELCOME_AT + i as f32 * m::GET_STARTED_WORD_STAGGER, m::GET_STARTED_WORD_REVEAL)
    }

    pub fn cast(self, i: usize) -> f32 {
        span(self.0, Self::cast_at(i), m::GET_STARTED_CAST_POP)
    }

    pub fn trail(self, i: usize) -> f32 {
        linear(self.0, Self::cast_at(i + 1), m::GET_STARTED_TRAIL)
    }

    pub fn leave(self) -> f32 {
        span(self.0, m::GET_STARTED_LEAVE_AT, m::GET_STARTED_LEAVE)
    }

    pub fn lift(self) -> f32 {
        span(self.0, m::GET_STARTED_LEAVE_AT, m::GET_STARTED_LIFT)
    }

    pub fn flight(self) -> f32 {
        m::MORPH_EASE.apply(linear(self.0, m::GET_STARTED_LEAVE_AT, m::GET_STARTED_FLIGHT))
    }

    pub fn title(self) -> f32 {
        span(self.0, m::GET_STARTED_CARDS_AT, m::GET_STARTED_REVEAL)
    }

    pub fn card(self, i: usize) -> f32 {
        span(self.0, Self::card_at(i), m::GET_STARTED_CARD_GROW)
    }

    pub fn content(self, i: usize) -> f32 {
        span(self.0, Self::card_at(i) + m::GET_STARTED_CARD_GROW * 0.6, m::GET_STARTED_REVEAL)
    }

    pub fn action(self, cards: usize) -> f32 {
        span(self.0, Self::action_at(cards), m::GET_STARTED_REVEAL)
    }

    pub fn cast_at(i: usize) -> f32 {
        m::GET_STARTED_CAST_AT + i as f32 * m::GET_STARTED_CAST_STAGGER
    }

    pub fn card_at(i: usize) -> f32 {
        m::GET_STARTED_CARDS_AT + (i + 1) as f32 * m::GET_STARTED_CARD_STAGGER
    }

    pub fn action_at(cards: usize) -> f32 {
        Self::card_at(cards) + m::GET_STARTED_CARD_GROW
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_starts_dark_and_bare() {
        let b = Beat(0.0);
        assert_eq!(b.darken(), 0.0);
        assert_eq!((b.word(0), b.cast(0), b.card(0), b.action(4)), (0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn the_greeting_then_the_cast_then_the_cards() {
        let first_word = (0..).map(|f| f as f32 / 100.0).find(|t| Beat(*t).word(0) > 0.0).expect("word");
        let first_cast = (0..).map(|f| f as f32 / 100.0).find(|t| Beat(*t).cast(0) > 0.0).expect("cast");
        let first_card = (0..).map(|f| f as f32 / 100.0).find(|t| Beat(*t).card(0) > 0.0).expect("card");
        assert!(first_word < first_cast && first_cast < first_card);
        assert!(Beat(first_word).darken() > 0.5, "words land on the dark");
        let full_cast = Beat(m::GET_STARTED_LEAVE_AT);
        assert!((0..4).all(|i| full_cast.cast(i) == 1.0), "everyone arrives before leaving");
        assert!((0..3).all(|i| full_cast.trail(i) == 1.0), "trails drawn before leaving");
        assert!(full_cast.leave() == 0.0 && full_cast.lift() == 0.0);
    }

    #[test]
    fn a_trail_waits_for_both_ends() {
        for i in 0..3 {
            let t = (0..).map(|f| f as f32 / 100.0).find(|t| Beat(*t).trail(i) > 0.0).expect("trail");
            assert!(Beat(t).cast(i + 1) < 0.1, "drawn as the next member arrives");
            assert!(Beat(t).cast(i) > 0.5);
        }
    }

    #[test]
    fn everything_has_settled_when_the_opening_ends() {
        let b = Beat(m::GET_STARTED_OPEN);
        assert_eq!((b.leave(), b.lift(), b.flight(), b.title()), (1.0, 1.0, 1.0, 1.0));
        assert!((0..4).all(|i| b.card(i) == 1.0 && b.content(i) == 1.0));
        assert_eq!(b.action(4), 1.0);
        assert!(Beat(m::GET_STARTED_OPEN - 0.05).action(4) < 1.0, "Continue is the last to land");
    }

    #[test]
    fn cards_follow_each_other() {
        let b = Beat(m::GET_STARTED_CARDS_AT + 0.3);
        assert!(b.card(0) > b.card(1) && b.card(1) > b.card(2));
        assert!(b.content(0) <= b.card(0));
    }
}
