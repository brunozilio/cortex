//! Native user input, isolated-world capture and the public host boundary.
use super::{BrowserOp, Error, Harness, Locator, reference};

pub(super) fn check(h: &mut Harness) -> Result<(), Error> {
    h.evaluate(r#"document.body.insertAdjacentHTML('afterbegin','<form onsubmit="event.preventDefault()"><input aria-label="Capture username" autocomplete="username"><input aria-label="Capture password" type="password" autocomplete="current-password"></form>');true"#)?;
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let user = reference(&snapshot, "Capture username")?;
    let password = reference(&snapshot, "Capture password")?;
    h.command(BrowserOp::Type {
        target: Some(Locator::Ref(user)),
        text: "capture-fixture-user".into(),
        submit: false,
    })?;
    h.command(BrowserOp::Type {
        target: Some(Locator::Ref(password)),
        text: "capture-fixture-password".into(),
        submit: true,
    })?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while h.submissions.is_empty() && std::time::Instant::now() < deadline {
        h.command(BrowserOp::Snapshot)?;
    }
    let sent = h.submissions.first().ok_or("native sign-in capture did not reach the host")?;
    assert_eq!(sent.username, "capture-fixture-user");
    assert_eq!(sent.password.as_deref(), Some("capture-fixture-password"));
    assert!(!format!("{sent:?}").contains("capture-fixture-password"));
    assert!(!h.command(BrowserOp::Snapshot)?.text.contains("capture-fixture-password"));
    assert!(
        h.command(BrowserOp::FillLogin { item: "fixture".into() }).is_err(),
        "raw agent operation bypassed vault routing"
    );
    eprintln!("Capture acceptance passed: trusted input, native event, redacted Debug and vault-only sign-in route");
    Ok(())
}
