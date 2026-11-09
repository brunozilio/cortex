//! Clipboard edit results must belong to the command that requested them.
use super::*;
use superfik_browser::{EditCommand, PageInput};

pub(super) fn check(h: &mut Harness) -> Result<(), Error> {
    h.evaluate("document.body.prepend(Object.assign(document.createElement('input'),{id:'clipboard',value:'Linux clipboard fixture'}));document.getElementById('clipboard').focus();true")?;
    for command in [EditCommand::SelectAll, EditCommand::Copy] {
        h.host.send(Call::Input(h.page, PageInput::Edit(command)));
    }
    assert_eq!(copied(h)?, "Linux clipboard fixture", "another edit's result was copied");
    for command in [EditCommand::Paste("replacement".into()), EditCommand::SelectAll, EditCommand::Cut] {
        h.host.send(Call::Input(h.page, PageInput::Edit(command)));
    }
    assert_eq!(copied(h)?, "replacement", "paste/select results replaced the cut's text");
    assert_eq!(h.evaluate("document.getElementById('clipboard').value===''")?, "true");
    eprintln!("Clipboard acceptance passed: select/copy and paste/select/cut preserve command identity");
    Ok(())
}

fn copied(h: &mut Harness) -> Result<String, Error> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        h.event_loop.pump_app_events(Some(Duration::from_millis(8)), &mut h.shell);
        for (id, event) in h.host.pump(&h.device, &h.queue) {
            if id == h.page
                && let PageEvent::Copied(text) = event
            {
                return Ok(text);
            }
        }
    }
    Err("the page did not return clipboard text".into())
}
