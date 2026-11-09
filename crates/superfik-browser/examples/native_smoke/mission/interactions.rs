//! Native input, nested origins, actionability and bounded snapshot reads.
use super::{BrowserOp, Error, Harness, Locator, reference};

pub(super) fn check(h: &mut Harness) -> Result<(), Error> {
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let frame = reference(&snapshot, "Frame action")?;
    h.command(BrowserOp::Click { target: Locator::Ref(frame) })?;
    assert_eq!(h.evaluate("window.frameClicked")?, "true", "cross-origin click missed the child");
    h.evaluate(r#"document.body.prepend(Object.assign(document.createElement('input'),{id:'edit',value:'old text',ariaLabel:'Mission text',oninput:e=>window.trustedInput=e.isTrusted}));true"#)?;
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let edit = reference(&snapshot, "Mission text")?;
    h.command(BrowserOp::Type { target: Some(Locator::Ref(edit)), text: "replacement text".into(), submit: false })?;
    assert_eq!(
        h.evaluate("document.getElementById('edit').value==='replacement text' && window.trustedInput===true")?,
        "true",
        "text was appended or input was untrusted"
    );
    let stable = reference(&snapshot, "Stable action")?;
    h.evaluate("document.getElementById('stable').disabled=true;true")?;
    assert!(h.command(BrowserOp::Click { target: Locator::Ref(stable) }).is_err(), "disabled element was clicked");
    h.evaluate(r#"document.getElementById('stable').disabled=false;document.body.append(Object.assign(document.createElement('div'),{id:'cover',style:'position:fixed;inset:0;z-index:10000;background:white'}));true"#)?;
    assert!(h.command(BrowserOp::Click { target: Locator::Ref(stable) }).is_err(), "covered element was clicked");
    h.evaluate("document.getElementById('cover').remove();document.body.append(Object.assign(document.createElement('p'),{textContent:'snapshot-content '.repeat(4000)+'END OF LONG SNAPSHOT'}));true")?;
    let first = h.command(BrowserOp::Snapshot)?.text;
    assert!(first.contains("More content:"), "long snapshot was silently truncated");
    let mut page = first;
    while let Some(tail) = page.split("browser_snapshot offset=").nth(1) {
        let offset = tail.split(';').next().ok_or("offset")?.parse()?;
        page = h.command(BrowserOp::SnapshotPage { offset })?.text;
    }
    assert!(page.contains("END OF LONG SNAPSHOT"), "snapshot continuation lost content");
    h.command(BrowserOp::Reload)?;
    assert!(
        h.command(BrowserOp::Click { target: Locator::Ref(stable) }).is_err(),
        "navigation reused an old reference"
    );
    eprintln!(
        "Interaction acceptance passed: cross-origin click, trusted replacement, actionability, pagination and navigation invalidation"
    );
    Ok(())
}
