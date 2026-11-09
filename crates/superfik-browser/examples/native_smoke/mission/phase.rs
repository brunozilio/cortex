//! Bound child lifetime, including a broken modal/native shutdown path.
use super::{Error, Path};
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};
pub(super) fn run(phase: &str, root: &Path) -> Result<(), Error> {
    let mut child = std::process::Command::new(std::env::current_exe()?)
        .args([phase, &root.to_string_lossy()])
        .args(["-ApplePersistenceIgnoreState", "YES"])
        .process_group(0)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(75);
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("agent acceptance failed ({phase}): {status}").into())
            };
        }
        if Instant::now() >= deadline {
            let _ = std::process::Command::new("/bin/kill").args(["-KILL", &format!("-{}", child.id())]).status();
            let _ = child.wait();
            return Err(format!("native acceptance child timed out ({phase})").into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
