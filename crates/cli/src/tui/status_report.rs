use domain::command::mode_label;

use super::app::App;

pub fn show(app: &mut App) {
    let text = report(app);
    app.transcript.notice(text);
}

fn report(app: &App) -> String {
    let session = app.session.id().map_or("not started".to_owned(), |id| id.0);
    let model = app.model().map_or("none".to_owned(), |m| format!("{} ({})", m.label, m.id));
    let mut text = format!(
        "{brand_name} {}\n  Model    {model}\n  Mode     {}\n  Effort   {}\n  Folder   {}\n  Session  {session}",
        env!("CARGO_PKG_VERSION"),
        mode_label(app.mode()),
        app.effort_label(),
        app.cwd.display(),
        brand_name = brand::NAME
    );
    for dir in app.session.added_dirs() {
        text.push_str(&format!("\n  Added    {}", dir.display()));
    }
    if app.transcript.cost > 0.0 {
        text.push_str(&format!("\n  Cost     ${:.4}", app.transcript.cost));
    }
    text
}
