use super::app::App;

pub fn show(app: &mut App) {
    app.transcript.notice(concat!(
        "New lines: Alt+Enter or Ctrl+J work everywhere. For Shift+Enter, use a terminal that reports it \
         (Ghostty, kitty, WezTerm, iTerm2 with \"Report modifiers using CSI u\"), or map Shift+Enter to \
         send \\n in your terminal's key settings. ",
        brand::name!(),
        " asks the terminal for these keys when it can."
    ));
}
