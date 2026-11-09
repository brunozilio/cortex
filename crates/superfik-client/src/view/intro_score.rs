//! Get Started's sound: each beat of the opening is voiced by a cue of the
//! current pack, played as the opening passes it. The moments come from the
//! choreography (`Beat`), so sound and motion never drift apart.

use superfik_sound::Cue;
use superfik_ui::components::get_started::Beat;
use superfik_ui::theme::motion as m;

const CAST: usize = 4;
const CARDS: usize = 4;

fn score() -> Vec<(f32, Cue)> {
    let mut score = vec![(m::GET_STARTED_WELCOME_AT, Cue::Notification)];
    score.extend((0..CAST).map(|i| (Beat::cast_at(i), Cue::Tap)));
    score.push((m::GET_STARTED_LEAVE_AT, Cue::Swipe));
    score.extend((0..CARDS).map(|i| (Beat::card_at(i), Cue::Type)));
    score.push((Beat::action_at(CARDS), Cue::Select));
    score
}

#[derive(Debug, Default)]
pub struct IntroScore {
    heard: Option<f32>,
}

impl IntroScore {
    pub fn advance(&mut self, elapsed: Option<f32>) -> Option<Cue> {
        let Some(now) = elapsed else {
            self.heard = None;
            return None;
        };
        let from = self.heard.replace(now).unwrap_or(f32::NEG_INFINITY);
        score().into_iter().filter(|(at, _)| from < *at && *at <= now).map(|(_, cue)| cue).next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(score: &mut IntroScore) -> Vec<Cue> {
        (0..=(m::GET_STARTED_OPEN * 120.0) as usize + 60)
            .filter_map(|f| score.advance(Some(f as f32 / 120.0)))
            .collect()
    }

    #[test]
    fn every_beat_sounds_once_in_order() {
        use Cue::*;
        let expected = [Notification, Tap, Tap, Tap, Tap, Swipe, Type, Type, Type, Type, Select];
        assert_eq!(heard(&mut IntroScore::default()), expected);
    }

    #[test]
    fn the_score_keeps_to_the_choreography() {
        let score = score();
        assert!(score.windows(2).all(|w| w[0].0 < w[1].0), "in time order");
        let last = score.last().map(|(at, _)| *at).unwrap_or_default();
        assert!(last < m::GET_STARTED_OPEN, "Continue sounds as it lands, within the opening");
    }

    #[test]
    fn a_long_frame_plays_only_the_latest_moment() {
        let mut score = IntroScore::default();
        assert_eq!(score.advance(Some(0.0)), None, "silent as it opens");
        assert_eq!(score.advance(Some(m::GET_STARTED_LEAVE_AT)), Some(Cue::Swipe));
        assert_eq!(score.advance(Some(m::GET_STARTED_LEAVE_AT + 0.01)), None);
    }

    #[test]
    fn closing_rewinds_it_for_the_next_opening() {
        let mut score = IntroScore::default();
        heard(&mut score);
        assert_eq!(score.advance(None), None);
        assert_eq!(heard(&mut score).len(), 11, "plays again in full");
    }
}
