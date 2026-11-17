use super::app::App;

pub fn vim(app: &mut App) {
    app.prompt.vim = if app.prompt.vim.is_some() { None } else { Some(super::vim::Vim::default()) };
    app.saver.remember("vim", if app.prompt.vim.is_some() { "on" } else { "off" });
    app.transcript.notice(if app.prompt.vim.is_some() {
        "Vim keys on: Esc for normal mode, i to insert."
    } else {
        "Vim keys off."
    });
}
