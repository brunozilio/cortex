use super::{BrowserOp, Error, Harness, Locator, reference};
pub(super) fn existing_vault_fill(h: &mut Harness) -> Result<(), Error> {
    use superfik_browser::{BrowserHost, Call, PageEvent};
    use superfik_core::autofill::{FillOutcome, LoginFill};
    use winit::platform::pump_events::EventLoopExtPumpEvents;
    h.evaluate(r#"document.body.insertAdjacentHTML('afterbegin','<form><label>Username<input id="fixture-user" autocomplete="username"></label><label>Password<input id="fixture-password" type="password" autocomplete="current-password"></label></form>');true"#)?;
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let stable = reference(&snapshot, "Stable action")?;
    let url: String = serde_json::from_str(&h.evaluate("location.href")?)?;
    h.host.send(Call::FillLogin {
        id: h.page,
        url,
        fill: LoginFill { username: Some("fixture-user".into()), password: Some("fixture-password".into()) },
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut completed = false;
    while std::time::Instant::now() < deadline && !completed {
        h.event_loop.pump_app_events(Some(std::time::Duration::from_millis(8)), &mut h.shell);
        for (_, event) in h.host.pump(&h.device, &h.queue) {
            if let PageEvent::LoginFilled(outcome) = event {
                assert_eq!(outcome, FillOutcome::Filled, "existing vault fill failed");
                completed = true;
            }
        }
    }
    assert!(completed, "existing vault fill did not answer");
    assert_eq!(h.evaluate("document.getElementById('fixture-user').value==='fixture-user' && document.getElementById('fixture-password').value==='fixture-password'")?,"true");
    h.command(BrowserOp::Click { target: Locator::Ref(stable) })?;
    assert_eq!(h.evaluate("window.clicked")?, "\"stable\"");
    assert!(!h.command(BrowserOp::Snapshot)?.text.contains("value=\"fixture-password\""));
    eprintln!("Existing vault acceptance passed: fill result, native reference and input compatibility");
    Ok(())
}

pub(super) fn close_while_opening(h: &mut Harness) -> Result<(), Error> {
    use superfik_browser::{BrowserHost, Call, PageEvent};
    use winit::platform::pump_events::EventLoopExtPumpEvents;
    let request = superfik_core::BrowseId("cancel-opening".into());
    h.host.send(Call::Agent { id: h.page, request: request.clone(), op: BrowserOp::Snapshot });
    h.host.send(Call::Close(h.page));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        h.event_loop.pump_app_events(Some(std::time::Duration::from_millis(8)), &mut h.shell);
        for (_, event) in h.host.pump(&h.device, &h.queue) {
            if let PageEvent::Answered { request: received, answer } = event
                && received == request
            {
                assert!(answer.is_err(), "closed opening reported success");
                eprintln!("Close acceptance passed: pending opening receives cancellation");
                return Ok(());
            }
        }
    }
    Err("closing during startup silently discarded the pending request".into())
}

pub(super) fn dialogs(h: &mut Harness) -> Result<(), Error> {
    let query: BrowserOp = serde_json::from_value(serde_json::json!({"dialog":{}}))?;
    assert!(h.command(query)?.text.contains("null"));
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let target = reference(&snapshot, "Ask confirmation")?;
    let opened = h.command(BrowserOp::Click { target: Locator::Ref(target) })?;
    assert!(opened.text.to_lowercase().contains("dialog"), "dialog not reported: {opened:?}");
    let dismiss: BrowserOp = serde_json::from_value(serde_json::json!({"dialog":{"accept":false}}))?;
    h.command(dismiss)?;
    assert_eq!(h.evaluate("window.confirmed")?, "false", "dialog was implicitly accepted");
    h.command(BrowserOp::Click { target: Locator::Ref(target) })?;
    h.command(serde_json::from_value(serde_json::json!({"dialog":{"accept":true}}))?)?;
    assert_eq!(h.evaluate("window.confirmed")?, "true", "explicit confirmation did not reach the page");
    h.command(BrowserOp::Evaluate { script: "window.prompted=prompt('Fixture text','default')".into() })?;
    h.command(serde_json::from_value(serde_json::json!({"dialog":{"accept":true,"prompt":"mission text"}}))?)?;
    assert_eq!(h.evaluate("window.prompted")?, "\"mission text\"");
    eprintln!("Dialog acceptance passed: explicit dismiss, accept and prompt text");
    Ok(())
}

pub(super) fn files(h: &mut Harness) -> Result<(), Error> {
    let files: BrowserOp = serde_json::from_str("\"files\"")?;
    let area: serde_json::Value = serde_json::from_str(&h.command(files)?.text)?;
    let uploads = std::path::PathBuf::from(area["upload_directory"].as_str().ok_or("upload directory")?);
    std::fs::write(uploads.join("report.txt"), "native upload bytes")?;
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let target = reference(&snapshot, "Mission upload")?;
    let upload: BrowserOp =
        serde_json::from_value(serde_json::json!({"upload":{"reference":target,"paths":["report.txt"]}}))?;
    h.command(upload)?;
    assert_eq!(h.evaluate("document.getElementById('upload').files[0].text()")?, "\"native upload bytes\"");
    let trigger = reference(&snapshot, "Attach staged report")?;
    let custom: BrowserOp =
        serde_json::from_value(serde_json::json!({"upload":{"reference":trigger,"paths":["report.txt"]}}))?;
    h.command(custom)?;
    assert_eq!(h.evaluate("document.getElementById('hidden-upload').files[0].text()")?, "\"native upload bytes\"");
    let outside = uploads.parent().ok_or("parent")?.join("escape.txt");
    std::fs::write(&outside, "outside staging directory")?;
    for path in ["../escape.txt", outside.to_str().ok_or("path")?] {
        let upload: BrowserOp =
            serde_json::from_value(serde_json::json!({"upload":{"reference":target,"paths":[path]}}))?;
        assert!(h.command(upload).is_err(), "upload escaped the staging directory");
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, uploads.join("escape-link.txt"))?;
        let upload: BrowserOp =
            serde_json::from_value(serde_json::json!({"upload":{"reference":target,"paths":["escape-link.txt"]}}))?;
        assert!(h.command(upload).is_err(), "symlink escaped the staging directory");
    }
    let download = reference(&snapshot, "Download report")?;
    let mut downloaded = Vec::new();
    for _ in 0..2 {
        h.command(BrowserOp::Click { target: Locator::Ref(download) })?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
        loop {
            let area: serde_json::Value = serde_json::from_str(&h.command(serde_json::from_str("\"files\"")?)?.text)?;
            let completed: Vec<_> = area["downloads"]
                .as_array()
                .ok_or("downloads")?
                .iter()
                .filter(|item| item["state"] == "complete")
                .collect();
            if completed.len() > downloaded.len() {
                let path = std::path::PathBuf::from(
                    completed.last().ok_or("completed")?["path"].as_str().ok_or("download path")?,
                );
                assert_eq!(std::fs::read_to_string(&path)?, "downloaded mission bytes");
                assert!(!downloaded.contains(&path), "a repeated download overwrote an earlier file");
                downloaded.push(path);
                break;
            }
            assert!(std::time::Instant::now() < deadline, "download did not complete: {area}");
        }
    }
    eprintln!("File acceptance passed: native upload, path confinement and repeated downloads without overwrite");
    Ok(())
}

pub(super) fn oauth(h: &mut Harness) -> Result<(), Error> {
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let button = reference(&snapshot, "Sign in pop-up")?;
    h.command(BrowserOp::Click { target: Locator::Ref(button) })?;
    assert_eq!(
        h.evaluate("location.pathname === '/' && window.opened === true")?,
        "true",
        "popup must preserve its native opener and origin page"
    );
    let tabs: serde_json::Value = serde_json::from_str(&h.command(BrowserOp::Tabs)?.text)?;
    let list = tabs["tabs"].as_array().ok_or("missing tabs")?;
    assert_eq!(list.len(), 2, "popup was not adopted as a tab: {tabs}");
    let popup = list.iter().find(|tab| tab["opener"] == false).ok_or("missing popup")?["id"]
        .as_u64()
        .ok_or("missing popup id")? as u32;
    h.command(BrowserOp::SwitchTab { tab: popup })?;
    let wait: BrowserOp = serde_json::from_value(serde_json::json!({"wait":{"url":"/oauth","timeout_ms":2000}}))?;
    h.command(wait)?;
    assert!(super::super::BrowserHost::frame(&h.host, h.page).is_some(), "selected popup is not rendered in the card");
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let finish = reference(&snapshot, "Finish sign in")?;
    h.command(BrowserOp::Click { target: Locator::Ref(finish) })?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut restored = String::new();
    while std::time::Instant::now() < deadline {
        restored = h.evaluate("window.authenticated === true && location.pathname === '/'").unwrap_or_default();
        if restored == "true" {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert_eq!(restored, "true", "postMessage or opener restoration failed");
    let tabs: serde_json::Value = serde_json::from_str(&h.command(BrowserOp::Tabs)?.text)?;
    assert_eq!(tabs["tabs"].as_array().ok_or("tabs")?.len(), 1);
    eprintln!(
        "OAuth acceptance passed: native opener, visible popup tab, postMessage, window.close and return to original page"
    );
    Ok(())
}
