use super::app::App;

pub fn mouse(app: &mut App) {
    app.mouse = !app.mouse;
    app.saver.remember("mouse", if app.mouse { "on" } else { "off" });
    app.transcript.notice(if app.mouse {
        "The wheel scrolls now. Select text with Shift held (Option on macOS)."
    } else {
        "The wheel is the terminal's again."
    });
}

pub fn vim(app: &mut App) {
    app.vim = if app.vim.is_some() { None } else { Some(super::vim::Vim::default()) };
    app.saver.remember("vim", if app.vim.is_some() { "on" } else { "off" });
    app.transcript.notice(if app.vim.is_some() {
        "Vim keys on: Esc for normal mode, i to insert."
    } else {
        "Vim keys off."
    });
}
