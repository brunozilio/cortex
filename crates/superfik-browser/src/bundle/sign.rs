//! Signs the development bundle with the project's Developer ID, so a
//! `cargo run` app has a team and the keychain group Chromium's Touch ID
//! passkeys need (granted by the embedded provisioning profile).
//!
//! Each helper and the app are signed apart (they differ in identifier and
//! entitlements), helpers first and in parallel. No hardened runtime and no
//! timestamp: the bundle never leaves this machine. CI and development without
//! the identity use ad hoc signatures, without provisioned Keychain groups.

mod adhoc;
pub use adhoc::AdHoc;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Layout, passkeys};

pub const IDENTITY: &str = "Developer ID Application: Bruno Zilio (F3BSQ3FZTF)";
const PROFILE: &[u8] = include_bytes!("../../../../assets/macos/Superfik-dev.provisionprofile");

pub trait Signer: Sync {
    fn provisioned(&self) -> bool {
        true
    }
    fn available(&self) -> bool;
    fn sign(&self, path: &Path, entitlements: Option<&Path>) -> io::Result<()>;
}

#[derive(Debug)]
pub struct Codesign;

impl Signer for Codesign {
    fn available(&self) -> bool {
        Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).contains(&format!("\"{IDENTITY}\"")))
    }

    fn sign(&self, path: &Path, entitlements: Option<&Path>) -> io::Result<()> {
        sign_with(IDENTITY, path, entitlements)
    }
}

fn sign_with(identity: &str, path: &Path, entitlements: Option<&Path>) -> io::Result<()> {
    let mut command = Command::new("codesign");
    command.args(["--force", "--sign", identity]);
    if let Some(entitlements) = entitlements {
        command.arg("--entitlements").arg(entitlements);
    }
    let out = command.arg(path).output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("codesign {}: {}", path.display(), String::from_utf8_lossy(&out.stderr).trim())))
    }
}

pub fn entitlements() -> String {
    let [group, keys] = passkeys::groups();
    let credentials = superfik_keychain::ACCESS_GROUP;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>keychain-access-groups</key>
	<array><string>{group}</string><string>{keys}</string><string>{credentials}</string></array>
</dict>
</plist>
"#
    )
}

pub fn sign(layout: &Layout, signer: &dyn Signer) -> io::Result<bool> {
    if !signer.available() {
        return Ok(false);
    }
    let profile = layout.app.join("Contents/embedded.provisionprofile");
    let granted = if signer.provisioned() {
        fs::write(&profile, PROFILE)?;
        let path = entitlements_path(layout);
        fs::write(&path, entitlements())?;
        Some(path)
    } else {
        if profile.exists() {
            fs::remove_file(profile)?;
        }
        None
    };
    std::thread::scope(|scope| {
        let nested = layout.helpers.iter().map(|(bundle, _)| bundle).chain([&layout.framework]);
        let signing: Vec<_> = nested.map(|bundle| scope.spawn(move || signer.sign(bundle, None))).collect();
        signing.into_iter().try_for_each(|s| s.join().unwrap_or_else(|_| Err(io::Error::other("signing panicked"))))
    })?;
    signer.sign(&layout.app, granted.as_deref())?;
    Ok(true)
}

fn entitlements_path(layout: &Layout) -> PathBuf {
    layout.app.with_extension("entitlements")
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct Recorder {
        available: bool,
        signed: Mutex<Vec<(PathBuf, Option<PathBuf>)>>,
    }

    impl Recorder {
        fn new(available: bool) -> Self {
            Self { available, signed: Mutex::new(Vec::new()) }
        }
    }

    impl Signer for Recorder {
        fn available(&self) -> bool {
            self.available
        }

        fn sign(&self, path: &Path, entitlements: Option<&Path>) -> io::Result<()> {
            self.signed
                .lock()
                .map_err(|_| io::Error::other("poisoned"))?
                .push((path.to_path_buf(), entitlements.map(Path::to_path_buf)));
            Ok(())
        }
    }

    fn bundle(dir: &Path) -> io::Result<Layout> {
        let layout = Layout::new(dir);
        fs::create_dir_all(layout.app.join("Contents"))?;
        Ok(layout)
    }

    #[test]
    fn the_framework_and_helpers_are_signed_before_the_app_which_alone_gets_entitlements() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let layout = bundle(dir.path())?;
        let signer = Recorder::new(true);
        assert!(sign(&layout, &signer)?);
        let signed = signer.signed.into_inner().map_err(|_| io::Error::other("poisoned"))?;
        assert_eq!(signed.len(), 7);
        assert!(signed[..6].iter().all(|(path, e)| path != &layout.app && e.is_none()));
        assert!(signed[..6].iter().any(|(path, _)| path == &layout.framework));
        assert_eq!(signed[6], (layout.app.clone(), Some(entitlements_path(&layout))));
        Ok(())
    }

    #[test]
    fn the_profile_goes_in_and_the_entitlements_stay_out_of_the_bundle() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let layout = bundle(dir.path())?;
        sign(&layout, &Recorder::new(true))?;
        assert_eq!(fs::read(layout.app.join("Contents/embedded.provisionprofile"))?, PROFILE);
        let entitlements = entitlements_path(&layout);
        assert!(!entitlements.starts_with(&layout.app));
        let granted = fs::read_to_string(entitlements)?;
        assert!(granted.contains("<string>F3BSQ3FZTF.superfik.webauthn.v1</string>"));
        assert!(granted.contains("<string>F3BSQ3FZTF.superfik.webauthn.v1-uvk</string>"));
        assert!(granted.contains("<string>F3BSQ3FZTF.superfik.credentials</string>"));
        Ok(())
    }

    #[test]
    fn without_the_identity_nothing_is_signed() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let layout = bundle(dir.path())?;
        let signer = Recorder::new(false);
        assert!(!sign(&layout, &signer)?);
        assert!(signer.signed.into_inner().map_err(|_| io::Error::other("poisoned"))?.is_empty());
        assert!(!layout.app.join("Contents/embedded.provisionprofile").exists());
        Ok(())
    }
}
