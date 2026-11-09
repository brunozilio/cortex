#![cfg(target_os = "macos")]

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use superfik_browser::bundle::{Layout, prepare, sign::Signer};

struct Counter(AtomicUsize);
impl Signer for Counter {
    fn available(&self) -> bool {
        true
    }
    fn sign(&self, _: &Path, _: Option<&Path>) -> io::Result<()> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn selecting_another_engine_replaces_the_cached_framework() -> io::Result<()> {
    let directory = tempfile::tempdir()?;
    let exe = directory.path().join("superfik");
    std::fs::write(&exe, b"same executable")?;
    let first = directory.path().join("engine-first");
    let second = directory.path().join("engine-second");
    for (path, content) in [(&first, b"first".as_slice()), (&second, b"second".as_slice())] {
        std::fs::create_dir_all(path)?;
        std::fs::write(path.join("engine-fixture"), content)?;
    }
    let layout = Layout::new(directory.path());
    let signer = Counter(AtomicUsize::new(0));
    prepare(&layout, &exe, &first, &signer)?;
    prepare(&layout, &exe, &second, &signer)?;
    assert_eq!(std::fs::read(layout.framework.join("engine-fixture"))?, b"second");
    Ok(())
}
