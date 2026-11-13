use super::Feature;
use crate::tui::app::App;

pub fn feature() -> Feature {
    Feature { status: Some(status), ..Feature::default() }
}

fn status(app: &App) -> Option<String> {
    if app.busy() || !super::on(app, domain::features::CACHE_WARNING) {
        return None;
    }
    let last = app.transcript.last_call.as_ref()?;
    domain::cache_window::marker(last, crate::tui::conversations::now())
}
