//! A browser's own application icon, for Settings › Browser: its `.app`
//! is found where macOS keeps applications and its `.icns` turned into a
//! PNG with `sips`.

use std::path::{Path, PathBuf};

use super::BrowserKind;

impl BrowserKind {
    fn app_name(self) -> &'static str {
        match self {
            BrowserKind::Chrome => "Google Chrome.app",
            BrowserKind::Dia => "Dia.app",
            BrowserKind::Arc => "Arc.app",
            BrowserKind::Brave => "Brave Browser.app",
            BrowserKind::Edge => "Microsoft Edge.app",
            BrowserKind::Chromium => "Chromium.app",
            BrowserKind::Vivaldi => "Vivaldi.app",
        }
    }

    pub fn app(self, home: &Path) -> Option<PathBuf> {
        [PathBuf::from("/Applications"), home.join("Applications")]
            .into_iter()
            .map(|dir| dir.join(self.app_name()))
            .find(|app| app.is_dir())
    }
}

fn icns(app: &Path) -> Option<PathBuf> {
    let plist = app.join("Contents/Info.plist");
    let output = std::process::Command::new("/usr/bin/plutil")
        .args(["-extract", "CFBundleIconFile", "raw", "-o", "-"])
        .arg(&plist)
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || name.is_empty() {
        return None;
    }
    Some(app.join("Contents/Resources").join(with_icns(&name)))
}

fn with_icns(name: &str) -> String {
    if name.ends_with(".icns") { name.to_owned() } else { format!("{name}.icns") }
}

pub fn icon_png(app: &Path, size: u32, out: &Path) -> Result<(), String> {
    let icns = icns(app).ok_or_else(|| format!("{} names no icon", app.display()))?;
    let status = std::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "png", "-Z", &size.to_string()])
        .arg(&icns)
        .arg("--out")
        .arg(out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("sips: {error}"))?;
    if status.success() { Ok(()) } else { Err(format!("sips could not read {}", icns.display())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_files_get_their_extension_once() {
        assert_eq!(with_icns("AppIcon"), "AppIcon.icns");
        assert_eq!(with_icns("app.icns"), "app.icns");
    }

    #[test]
    fn apps_are_found_in_the_users_applications_too() -> std::io::Result<()> {
        let home = tempfile::tempdir()?;
        std::fs::create_dir_all(home.path().join("Applications/Vivaldi.app"))?;
        assert_eq!(BrowserKind::Vivaldi.app(home.path()), Some(home.path().join("Applications/Vivaldi.app")));
        Ok(())
    }
}
