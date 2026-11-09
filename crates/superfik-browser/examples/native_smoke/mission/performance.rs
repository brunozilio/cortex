//! Fixed native workload. External OS sampling measures the entire process tree.
use super::{BrowserOp, Error, Harness, Locator, reference};
use std::time::{Duration, Instant};
use superfik_browser::BrowserHost;
use winit::platform::pump_events::EventLoopExtPumpEvents;

pub(super) fn run(h: &mut Harness) -> Result<(), Error> {
    if !cfg!(debug_assertions) {
        return Err("Performance comparison requires a debug build.".into());
    }
    let snapshot = h.command(BrowserOp::Snapshot)?.text;
    let target = reference(&snapshot, "Stable action")?;
    pump(h, Duration::from_secs(2));
    marker("startup", "end");
    marker("idle", "begin");
    pump(h, Duration::from_secs(8));
    marker("idle", "end");
    marker("actions", "begin");
    for _ in 0..10 {
        h.command(BrowserOp::Snapshot)?;
        h.command(BrowserOp::Click { target: Locator::Ref(target) })?;
        assert_eq!(h.evaluate("window.clicked==='stable'")?, "true");
    }
    marker("actions", "end");
    Ok(())
}
fn marker(phase: &str, state: &str) {
    eprintln!("SUPERFIK_PERF pid={} phase={phase} state={state}", std::process::id());
}
fn pump(h: &mut Harness, interval: Duration) {
    let until = Instant::now() + interval;
    while Instant::now() < until {
        h.event_loop.pump_app_events(Some(Duration::from_millis(16)), &mut h.shell);
        h.host.pump(&h.device, &h.queue);
    }
}
