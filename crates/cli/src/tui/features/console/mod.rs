use super::{Feature, Slash};
use crate::tui::app::App;

const NAME: &str = "console";

#[derive(Default)]
struct Served(Option<console::Running>);

pub fn feature() -> Feature {
    Feature {
        commands: vec![Slash { names: &["console"], about: "Open the web console in your browser", run }],
        ..Feature::default()
    }
}

fn run(app: &mut App, _argument: Option<String>) {
    let Some(served) = app.features.get::<Served>(NAME) else { return };
    if served.0.is_none() {
        let options = console::Options { assets: console::discover(), port: 0 };
        match console::start(options) {
            Ok(running) => served.0 = Some(running),
            Err(error) => return app.transcript.notice(format!("The console could not start: {error}")),
        }
    }
    let Some(url) = served.0.as_ref().map(console::Running::url) else { return };
    crate::login::open(&url);
    app.transcript.notice(format!("Console: {url}"));
}
