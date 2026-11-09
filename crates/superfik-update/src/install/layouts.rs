//! The two installs that update themselves: a macOS `.app` (a zip made by
//! `ditto`) and Linux's unpacked tarball (`superfik-X.Y.Z/usr/bin/…`).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{Commands, Layout};
use crate::{UpdateError, Version};

pub const TEAM_ID: &str = "F3BSQ3FZTF";

pub struct MacApp {
    commands: Box<dyn Commands>,
}

impl std::fmt::Debug for MacApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MacApp")
    }
}

impl MacApp {
    pub fn new(commands: Box<dyn Commands>) -> Self {
        Self { commands }
    }
}

impl Layout for MacApp {
    fn unpack(&self, archive: &Path, into: &Path) -> Result<(), UpdateError> {
        let ran = self.commands.run("/usr/bin/ditto", &["-x".into(), "-k".into(), archive.into(), into.into()])?;
        ran.success.then_some(()).ok_or_else(|| UpdateError::Install(format!("ditto: {}", ran.output.trim())))
    }

    fn check(&self, app: &Path, version: &Version) -> Result<(), UpdateError> {
        let verify = ["--verify".into(), "--deep".into(), "--strict".into(), app.into()];
        if !self.commands.run("/usr/bin/codesign", &verify)?.success {
            return Err(UpdateError::Forged);
        }
        let shown = self.commands.run("/usr/bin/codesign", &["-dv".into(), "--verbose=2".into(), app.into()])?;
        if team_of(&shown.output) != Some(TEAM_ID) {
            return Err(UpdateError::Forged);
        }
        let plist = std::fs::read_to_string(app.join("Contents/Info.plist")).map_err(UpdateError::install)?;
        match short_version(&plist) {
            Some(found) if found == version.to_string() => Ok(()),
            found => {
                Err(UpdateError::Install(format!("the update is {}, not {version}", found.unwrap_or("unversioned"))))
            }
        }
    }

    fn relaunch(&self, current: &Path) -> Vec<OsString> {
        vec!["/usr/bin/open".into(), current.into()]
    }
}

pub struct Tarball {
    commands: Box<dyn Commands>,
}

impl std::fmt::Debug for Tarball {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tarball")
    }
}

impl Tarball {
    pub fn new(commands: Box<dyn Commands>) -> Self {
        Self { commands }
    }
}

impl Layout for Tarball {
    fn unpack(&self, archive: &Path, into: &Path) -> Result<(), UpdateError> {
        let ran = self.commands.run("tar", &["-xzf".into(), archive.into(), "-C".into(), into.into()])?;
        ran.success.then_some(()).ok_or_else(|| UpdateError::Install(format!("tar: {}", ran.output.trim())))
    }

    fn check(&self, root: &Path, version: &Version) -> Result<(), UpdateError> {
        let named = root.file_name().is_some_and(|name| name.to_string_lossy() == format!("superfik-{version}"));
        if !named {
            return Err(UpdateError::Install(format!("the update is not {version}")));
        }
        if !root.join("usr/bin/superfik").is_file() {
            return Err(UpdateError::Install("the update has no superfik binary".into()));
        }
        Ok(())
    }

    fn relaunch(&self, current: &Path) -> Vec<OsString> {
        vec![current.join("usr/bin/superfik").into()]
    }
}

pub fn app_bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let shaped = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension().is_some_and(|extension| extension == "app");
    shaped.then(|| app.to_path_buf())
}

pub fn tarball_root_of(exe: &Path) -> Option<PathBuf> {
    let parent = exe.parent()?;
    let usr = if parent.file_name()? == "bin" {
        parent.parent()?
    } else if parent.file_name()? == "superfik" && parent.parent()?.file_name()? == "lib" {
        parent.parent()?.parent()?
    } else {
        return None;
    };
    let root = usr.parent()?;
    let shaped = usr.file_name()? == "usr" && root.file_name().is_some();
    shaped.then(|| root.to_path_buf())
}

pub fn team_of(codesign: &str) -> Option<&str> {
    codesign.lines().find_map(|line| line.trim().strip_prefix("TeamIdentifier=")).map(str::trim)
}

pub fn short_version(plist: &str) -> Option<&str> {
    let after = plist.split("<key>CFBundleShortVersionString</key>").nth(1)?;
    let value = after.trim_start().strip_prefix("<string>")?;
    value.split("</string>").next().map(str::trim)
}
