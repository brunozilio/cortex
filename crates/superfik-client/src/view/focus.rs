//! Focus on some nodes: while the `/` picker is open, the nodes it fills
//! stay and every other node (and its connector) fades out of the way.

use superfik_motion::Presence;
use superfik_ui::theme::motion;

use super::layout::NodeKey;

#[derive(Debug, Clone)]
pub struct Focus {
    keys: Vec<NodeKey>,
    presence: Presence,
}

impl Default for Focus {
    fn default() -> Self {
        Self { keys: Vec::new(), presence: Presence::new(false, motion::FOCUS_IN, motion::FOCUS_OUT) }
    }
}

impl Focus {
    pub fn sync(&mut self, keys: &[NodeKey], now: f64) {
        if !keys.is_empty() {
            self.keys = keys.to_vec();
        }
        self.presence.set(!keys.is_empty(), now);
    }

    pub fn opacity(&self, key: NodeKey, now: f64) -> f32 {
        if self.keys.contains(&key) {
            return 1.0;
        }
        1.0 - motion::ENTER.apply(self.presence.progress(now))
    }

    pub fn is_animating(&self, now: f64) -> bool {
        self.presence.is_animating(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_others_fade_out_and_back() {
        let mut focus = Focus::default();
        assert_eq!(focus.opacity(NodeKey::Memory, 0.0), 1.0);
        focus.sync(&[NodeKey::Skills, NodeKey::Commands], 0.0);
        let settled = f64::from(motion::FOCUS_IN) + 0.01;
        assert_eq!(focus.opacity(NodeKey::Skills, settled), 1.0);
        assert_eq!(focus.opacity(NodeKey::Commands, settled), 1.0, "every focused node stays");
        assert_eq!(focus.opacity(NodeKey::Memory, settled), 0.0);
        focus.sync(&[], settled);
        assert_eq!(focus.opacity(NodeKey::Memory, settled + f64::from(motion::FOCUS_OUT) + 0.01), 1.0);
    }
}
