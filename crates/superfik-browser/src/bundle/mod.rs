//! The `.app` CEF needs on macOS, for a bare `cargo run` binary.
//!
//! CEF only runs inside an application bundle: the framework sits in
//! `Contents/Frameworks` and its subprocesses are five `Helper` bundles
//! beside it. [`relaunch_in_bundle`] lays that out next to the binary
//! (`target/debug/Superfik.app`) — the executable cloned into every slot
//! (APFS clones cost no space; each is signed apart) and the framework
//! cloned from the one cargo downloaded (a symlink out of the bundle would
//! break its seal) — signs it ([`sign`]) and runs the bundled one in this
//! process's place ([`relaunch`]). A release `.app` is laid out the same way
//! with real copies ([`assemble_release`], run by `scripts/bundle-macos.sh`
//! through `xtask cef-bundle`).

mod assemble;
pub mod passkeys;
mod relaunch;
pub mod sign;

pub use assemble::{assemble, assemble_release};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const NAME: &str = "Superfik";
pub const SERVER_NAME: &str = "Superfik Server";
pub const IDENTIFIER: &str = "com.brunozilio.superfik.dev";
const FRAMEWORK: &str = "Chromium Embedded Framework.framework";
const HELPERS: [&str; 5] = ["Helper", "Helper (GPU)", "Helper (Renderer)", "Helper (Plugin)", "Helper (Alerts)"];

pub fn is_bundled(exe: &Path) -> bool {
    let macos = exe.parent();
    let contents = macos.and_then(Path::parent);
    let app = contents.and_then(Path::parent);
    let named = |p: Option<&Path>, name: &str| p.and_then(Path::file_name).is_some_and(|n| n == name);
    named(macos, "MacOS") && named(contents, "Contents") && app.and_then(Path::extension).is_some_and(|e| e == "app")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub app: PathBuf,
    pub executable: PathBuf,
    pub framework: PathBuf,
    pub helpers: Vec<(PathBuf, PathBuf)>,
}

impl Layout {
    pub fn new(dir: &Path) -> Self {
        Self::named(dir, NAME)
    }

    pub fn named(dir: &Path, name: &str) -> Self {
        Self::of(&dir.join(format!("{name}.app")), name)
    }

    pub fn of(app: &Path, executable: &str) -> Self {
        let app = app.to_path_buf();
        let frameworks = app.join("Contents/Frameworks");
        let helpers = HELPERS
            .iter()
            .map(|kind| {
                let name = format!("{executable} {kind}");
                let bundle = frameworks.join(format!("{name}.app"));
                let executable = bundle.join("Contents/MacOS").join(&name);
                (bundle, executable)
            })
            .collect();
        let executable = app.join("Contents/MacOS").join(executable);
        Self { executable, framework: frameworks.join(FRAMEWORK), app, helpers }
    }
}

pub fn info_plist(executable: &str, identifier: &str, helper: bool) -> String {
    let identifier = if helper { format!("{identifier}.helper") } else { identifier.to_owned() };
    let background = if helper { "\n\t<key>LSUIElement</key>\n\t<string>1</string>" } else { "" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleExecutable</key>
	<string>{executable}</string>
	<key>CFBundleIdentifier</key>
	<string>{identifier}</string>
	<key>CFBundleName</key>
	<string>{executable}</string>
	<key>CFBundleDisplayName</key>
	<string>{NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSMinimumSystemVersion</key>
	<string>11.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key>
	<true/>
	<key>NSBluetoothAlwaysUsageDescription</key>
	<string>Superfik uses Bluetooth so web pages can reach your security keys and passkeys.</string>
	<key>NSMicrophoneUsageDescription</key>
	<string>Superfik listens while the voice agent is on, and transcribes on this computer.</string>{background}
	<key>LSEnvironment</key>
	<dict>
		<key>MallocNanoZone</key>
		<string>0</string>
	</dict>
</dict>
</plist>
"#,
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn stamp(exe: &Path) -> io::Result<String> {
    let meta = fs::metadata(exe)?;
    let modified = meta.modified()?.duration_since(std::time::UNIX_EPOCH).map_err(io::Error::other)?;
    Ok(format!("{} {}", meta.len(), modified.as_nanos()))
}

fn stamp_path(layout: &Layout) -> PathBuf {
    layout.app.with_extension("stamp")
}

pub fn prepare(layout: &Layout, exe: &Path, framework: &Path, signer: &dyn sign::Signer) -> io::Result<()> {
    if let Some(dir) = layout.app.parent() {
        fs::create_dir_all(dir)?;
    }
    let lock = fs::File::create(layout.app.with_extension("lock"))?;
    lock.lock()?;
    let stamp =
        format!("{}\n{}\nprovisioned={}", stamp(exe)?, assemble::framework_stamp(framework)?, signer.provisioned());
    let path = stamp_path(layout);
    if layout.executable.exists() && fs::read_to_string(&path).ok().as_deref() == Some(stamp.as_str()) {
        return Ok(());
    }
    if path.exists() {
        fs::remove_file(&path)?;
    }
    assemble(layout, exe, framework)?;
    match sign::sign(layout, signer) {
        Ok(_) => fs::write(path, stamp),
        Err(error) => {
            tracing::warn!(%error, "the development bundle is not signed");
            Ok(())
        }
    }
}

pub fn relaunch_in_bundle() -> io::Result<()> {
    relaunch_in_bundle_named(NAME)
}

pub fn relaunch_in_bundle_named(name: &str) -> io::Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    if is_bundled(&exe) {
        return Ok(());
    }
    let dir = exe.parent().ok_or_else(|| io::Error::other("the executable has no directory"))?;
    let cef = cef_dll_sys::get_cef_dir().ok_or_else(|| io::Error::other("the CEF distribution was not found"))?;
    let layout = Layout::named(dir, name);
    use sign::Signer;
    let signer: &dyn Signer = if std::env::var("SUPERFIK_ADHOC").as_deref() == Ok("1") || !sign::Codesign.available() {
        &sign::AdHoc
    } else {
        &sign::Codesign
    };
    prepare(&layout, &exe, &cef.join(FRAMEWORK), signer)?;
    std::process::exit(relaunch::run_disclaimed(&layout.executable)?)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering::SeqCst;

    use super::*;

    #[test]
    fn a_binary_is_bundled_only_inside_an_app() {
        assert!(is_bundled(Path::new("/t/Superfik.app/Contents/MacOS/Superfik")));
        assert!(is_bundled(Path::new("/t/Superfik.app/Contents/Frameworks/Superfik Helper.app/Contents/MacOS/x")));
        assert!(!is_bundled(Path::new("/t/target/debug/superfik")));
        assert!(!is_bundled(Path::new("/t/Contents/MacOS/superfik")));
    }

    #[test]
    fn the_layout_names_helpers_after_the_app() {
        let layout = Layout::new(Path::new("/t"));
        assert_eq!(layout.executable, Path::new("/t/Superfik.app/Contents/MacOS/Superfik"));
        assert_eq!(
            layout.helpers[1].1,
            Path::new(
                "/t/Superfik.app/Contents/Frameworks/Superfik Helper (GPU).app/Contents/MacOS/Superfik Helper (GPU)"
            )
        );
        assert_eq!(layout.helpers.len(), 5);
    }

    #[test]
    fn the_servers_bundle_sits_beside_the_apps_with_its_own_helpers() {
        let layout = Layout::named(Path::new("/t"), SERVER_NAME);
        assert_eq!(layout.executable, Path::new("/t/Superfik Server.app/Contents/MacOS/Superfik Server"));
        assert!(layout.helpers[0].1.ends_with("Superfik Server Helper.app/Contents/MacOS/Superfik Server Helper"));
    }

    #[test]
    fn release_helpers_are_named_after_its_executable() {
        let layout = Layout::of(Path::new("/r/Superfik.app"), "superfik");
        assert_eq!(layout.executable, Path::new("/r/Superfik.app/Contents/MacOS/superfik"));
        assert!(layout.helpers[0].1.ends_with("superfik Helper.app/Contents/MacOS/superfik Helper"));
        assert!(
            info_plist("superfik Helper", "com.brunozilio.superfik", true).contains("com.brunozilio.superfik.helper")
        );
    }

    #[test]
    fn helpers_stay_out_of_the_dock() {
        assert!(info_plist("Superfik Helper", IDENTIFIER, true).contains("LSUIElement"));
        assert!(!info_plist(NAME, IDENTIFIER, false).contains("LSUIElement"));
    }

    struct Counter(std::sync::atomic::AtomicUsize);

    impl sign::Signer for Counter {
        fn available(&self) -> bool {
            true
        }

        fn sign(&self, _: &Path, _: Option<&Path>) -> io::Result<()> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn an_unchanged_binary_is_neither_copied_nor_signed_again() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("superfik");
        fs::write(&exe, b"v1")?;
        let framework = dir.path().join("cef").join(FRAMEWORK);
        fs::create_dir_all(&framework)?;
        let layout = Layout::new(dir.path());
        let signer = Counter(Default::default());
        prepare(&layout, &exe, &framework, &signer)?;
        prepare(&layout, &exe, &framework, &signer)?;
        assert_eq!(signer.0.load(std::sync::atomic::Ordering::SeqCst), 7);
        fs::write(&exe, b"v2 is longer")?;
        prepare(&layout, &exe, &framework, &signer)?;
        assert_eq!(signer.0.load(std::sync::atomic::Ordering::SeqCst), 14);
        assert_eq!(fs::read(&layout.executable)?, b"v2 is longer");
        Ok(())
    }

    #[derive(Default)]
    struct Probe {
        calls: AtomicUsize,
        failing: usize,
        active: AtomicUsize,
        peak: AtomicUsize,
    }

    impl sign::Signer for Probe {
        fn available(&self) -> bool {
            true
        }

        fn sign(&self, _: &Path, _: Option<&Path>) -> io::Result<()> {
            let call = self.calls.fetch_add(1, SeqCst);
            let active = self.active.fetch_add(1, SeqCst) + 1;
            self.peak.fetch_max(active, SeqCst);
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.active.fetch_sub(1, SeqCst);
            if call < self.failing { Err(io::Error::other("codesign failed")) } else { Ok(()) }
        }
    }

    fn fixture(dir: &Path) -> io::Result<(PathBuf, PathBuf)> {
        let exe = dir.join("superfik");
        fs::write(&exe, b"v1")?;
        let framework = dir.join("cef").join(FRAMEWORK);
        fs::create_dir_all(&framework)?;
        Ok((exe, framework))
    }

    #[test]
    fn a_bundle_that_failed_to_sign_is_signed_on_the_next_launch() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let (exe, framework) = fixture(dir.path())?;
        let layout = Layout::new(dir.path());
        let signer = Probe { failing: 7, ..Probe::default() };
        prepare(&layout, &exe, &framework, &signer)?;
        prepare(&layout, &exe, &framework, &signer)?;
        assert!(signer.calls.load(SeqCst) > 7);
        Ok(())
    }

    #[test]
    fn launches_at_once_prepare_the_bundle_one_after_the_other() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let (exe, framework) = fixture(dir.path())?;
        let signer = Probe::default();
        std::thread::scope(|scope| {
            let launches: Vec<_> =
                (0..3).map(|_| scope.spawn(|| prepare(&Layout::new(dir.path()), &exe, &framework, &signer))).collect();
            launches.into_iter().try_for_each(|l| l.join().unwrap_or_else(|_| Err(io::Error::other("panicked"))))
        })?;
        assert!(signer.peak.load(SeqCst) <= 6);
        assert_eq!(signer.calls.load(SeqCst), 7);
        Ok(())
    }
}
