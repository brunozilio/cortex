mod board;
mod chats;
pub(crate) mod follow;
mod refresh;

use super::{Feature, Slash};
use crate::tui::app::App;

const NAME: &str = "trace";

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash {
            names: &["trace", "console"],
            about: "Open the live trace of every session in the browser: runs, spans, cost, waits and checks",
            run: open,
        }],
        tick: Some(follow::tick),
        ..Feature::default()
    }
}

fn open(app: &mut App, _argument: Option<String>) {
    follow::open(app);
}
