//! Installing: finding the install, the layouts' checks, staging, and the
//! swap script run against temporary folders.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::*;

type Call = (String, Vec<OsString>);

#[derive(Clone, Default)]
struct Tools {
    answers: Arc<Mutex<Vec<Ran>>>,
    calls: Arc<Mutex<Vec<Call>>>,
}

impl Tools {
    fn answering(answers: &[(bool, &str)]) -> Self {
        let answers = answers.iter().rev().map(|(success, out)| Ran { success: *success, output: (*out).into() });
        Self { answers: Arc::new(Mutex::new(answers.collect())), calls: Arc::default() }
    }
}

impl Commands for Tools {
    fn run(&self, program: &str, args: &[OsString]) -> Result<Ran, UpdateError> {
        self.calls.lock().expect("lock").push((program.into(), args.to_vec()));
        Ok(self.answers.lock().expect("lock").pop().unwrap_or(Ran { success: true, output: String::new() }))
    }

    fn detach(&self, program: &str, args: &[OsString]) -> Result<(), UpdateError> {
        self.run(program, args).map(drop)
    }
}

const PLIST: &str = "<dict>\n  <key>CFBundleShortVersionString</key><string>0.15.0</string>\n</dict>";

fn mac_app(dir: &Path, plist: &str) -> PathBuf {
    let app = dir.join("Superfik.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).expect("mkdir");
    std::fs::write(app.join("Contents/Info.plist"), plist).expect("plist");
    app
}

#[test]
fn finds_the_app_bundle_or_the_unpacked_tarball() {
    let exe = Path::new("/Applications/Superfik.app/Contents/MacOS/superfik");
    assert_eq!(app_bundle_of(exe), Some(PathBuf::from("/Applications/Superfik.app")));
    assert_eq!(app_bundle_of(Path::new("/repo/target/debug/superfik")), None);
    let tar = Path::new("/home/ana/superfik-0.14.0/usr/bin/superfik");
    assert_eq!(tarball_root_of(tar), Some(PathBuf::from("/home/ana/superfik-0.14.0")));
    let real = Path::new("/home/ana/superfik-0.14.0/usr/lib/superfik/superfik");
    assert_eq!(tarball_root_of(real), Some(PathBuf::from("/home/ana/superfik-0.14.0")));
    assert_eq!(tarball_root_of(Path::new("/usr/bin/superfik")), None);
    assert_eq!(tarball_root_of(Path::new("/usr/lib/superfik/superfik")), None);
    assert_eq!(tarball_root_of(Path::new("/usr/local/bin/superfik")), None);
}

#[test]
fn reads_the_team_and_the_bundle_version() {
    let shown =
        "Executable=/x\nIdentifier=com.brunozilio.superfik\nTeamIdentifier=F3BSQ3FZTF\nSealed Resources version=2\n";
    assert_eq!(team_of(shown), Some(TEAM_ID));
    assert_eq!(team_of("TeamIdentifier=not set"), Some("not set"));
    assert_eq!(team_of("Signature=adhoc"), None);
    assert_eq!(short_version(PLIST), Some("0.15.0"));
    assert_eq!(short_version("<dict></dict>"), None);
}

#[test]
fn a_mac_app_signed_by_the_release_team_at_the_right_version_passes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = mac_app(dir.path(), PLIST);
    let tools = Tools::answering(&[(true, ""), (true, "TeamIdentifier=F3BSQ3FZTF\n")]);
    assert_eq!(MacApp::new(Box::new(tools.clone())).check(&app, &Version::new(0, 15, 0)), Ok(()));
    let calls = tools.calls.lock().expect("lock").clone();
    assert_eq!(
        calls[0].1,
        ["--verify", "--deep", "--strict"].map(OsString::from).into_iter().chain([app.into()]).collect::<Vec<_>>()
    );
}

#[test]
fn a_mac_app_that_fails_codesign_or_another_team_is_forged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = mac_app(dir.path(), PLIST);
    let v = Version::new(0, 15, 0);
    let broken = Tools::answering(&[(false, "invalid signature")]);
    assert_eq!(MacApp::new(Box::new(broken)).check(&app, &v), Err(UpdateError::Forged));
    let stranger = Tools::answering(&[(true, ""), (true, "TeamIdentifier=ABCDE12345\n")]);
    assert_eq!(MacApp::new(Box::new(stranger)).check(&app, &v), Err(UpdateError::Forged));
    let adhoc = Tools::answering(&[(true, ""), (true, "Signature=adhoc\n")]);
    assert_eq!(MacApp::new(Box::new(adhoc)).check(&app, &v), Err(UpdateError::Forged));
}

#[test]
fn a_mac_app_of_another_version_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = mac_app(dir.path(), PLIST);
    let tools = Tools::answering(&[(true, ""), (true, "TeamIdentifier=F3BSQ3FZTF")]);
    let checked = MacApp::new(Box::new(tools)).check(&app, &Version::new(0, 16, 0));
    assert!(matches!(checked, Err(UpdateError::Install(_))), "{checked:?}");
}

#[test]
fn a_tarball_must_be_named_for_its_version_and_hold_the_binary() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("superfik-0.15.0");
    let tarball = Tarball::new(Box::new(Tools::default()));
    assert!(tarball.check(&root, &Version::new(0, 15, 0)).is_err());
    std::fs::create_dir_all(root.join("usr/bin")).expect("mkdir");
    std::fs::write(root.join("usr/bin/superfik"), "").expect("binary");
    assert_eq!(tarball.check(&root, &Version::new(0, 15, 0)), Ok(()));
    assert!(tarball.check(&root, &Version::new(0, 15, 1)).is_err());
    assert_eq!(tarball.relaunch(&root), vec![OsString::from(root.join("usr/bin/superfik"))]);
}

struct FakeZip;

impl Layout for FakeZip {
    fn unpack(&self, _: &Path, into: &Path) -> Result<(), UpdateError> {
        mac_app(into, PLIST);
        std::fs::create_dir_all(into.join("__MACOSX")).map_err(|e| UpdateError::Install(e.to_string()))
    }

    fn check(&self, unpacked: &Path, version: &Version) -> Result<(), UpdateError> {
        (version.minor == 15 && unpacked.ends_with("Superfik.app")).then_some(()).ok_or(UpdateError::Forged)
    }

    fn relaunch(&self, current: &Path) -> Vec<OsString> {
        vec!["open".into(), current.into()]
    }
}

fn swap_installer(dir: &Path, tools: &Tools) -> SwapInstaller {
    let current = dir.join("Applications/Superfik.app");
    std::fs::create_dir_all(&current).expect("mkdir");
    SwapInstaller::new(current, Box::new(FakeZip), Box::new(tools.clone()), 4242, dir.join("tmp"))
}

#[cfg(unix)]
#[test]
fn stages_beside_the_install_and_starts_the_swap() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tools = Tools::default();
    let installer = swap_installer(dir.path(), &tools);
    let prepared = installer.stage(Path::new("/a.zip"), &Version::new(0, 15, 0)).expect("staged");
    let staged = dir.path().join("Applications/.superfik-update/Superfik.app");
    assert_eq!(prepared, Prepared { version: Version::new(0, 15, 0), staged: staged.clone() });
    installer.restart(&prepared).expect("restart");
    let calls = tools.calls.lock().expect("lock").clone();
    let script = PathBuf::from(&calls[0].1[0]);
    let own = script.parent().expect("its folder");
    assert_eq!(own.parent(), Some(dir.path().join("tmp").as_path()), "a folder of its own in the temp folder");
    assert!(own.file_name().is_some_and(|name| name.to_string_lossy().starts_with("superfik-swap-")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &Path| std::fs::metadata(path).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode(own), 0o700, "only this user can put files there");
        assert_eq!(mode(&script), 0o700);
    }
    let current = dir.path().join("Applications/Superfik.app");
    let expected: Vec<OsString> = vec![
        script.clone().into(),
        "4242".into(),
        current.clone().into(),
        staged.into(),
        "open".into(),
        current.into(),
    ];
    assert_eq!(calls, vec![("/bin/sh".to_owned(), expected)]);
    assert_eq!(std::fs::read_to_string(script).ok().as_deref(), Some(SWAP_SCRIPT));
}

#[cfg(unix)]
#[test]
fn a_script_planted_where_the_swap_used_to_go_is_neither_run_nor_written_through() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tools = Tools::default();
    let installer = swap_installer(dir.path(), &tools);
    let prepared = installer.stage(Path::new("/a.zip"), &Version::new(0, 15, 0)).expect("staged");
    let tmp = dir.path().join("tmp");
    std::fs::create_dir_all(&tmp).expect("mkdir");
    let victim = dir.path().join("victim");
    std::fs::write(&victim, "untouched").expect("write");
    std::os::unix::fs::symlink(&victim, tmp.join("superfik-swap-4242.sh")).expect("plant");
    installer.restart(&prepared).expect("restart");
    assert_eq!(std::fs::read_to_string(&victim).expect("read"), "untouched");
    let calls = tools.calls.lock().expect("lock").clone();
    let script = PathBuf::from(&calls[0].1[0]);
    assert_ne!(script, tmp.join("superfik-swap-4242.sh"));
    assert_eq!(std::fs::read_to_string(script).ok().as_deref(), Some(SWAP_SCRIPT));
}

#[test]
fn a_staged_install_that_fails_its_check_is_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let installer = swap_installer(dir.path(), &Tools::default());
    assert_eq!(installer.stage(Path::new("/a.zip"), &Version::new(0, 16, 0)), Err(UpdateError::Forged));
    assert!(!dir.path().join("Applications/.superfik-update").exists());
}

#[test]
fn restart_refuses_what_was_not_staged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tools = Tools::default();
    let installer = swap_installer(dir.path(), &tools);
    let elsewhere = Prepared { version: Version::new(0, 15, 0), staged: dir.path().join("Applications") };
    assert!(installer.restart(&elsewhere).is_err());
    assert!(tools.calls.lock().expect("lock").is_empty());
}

#[cfg(unix)]
mod swap_script;
