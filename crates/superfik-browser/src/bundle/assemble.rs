//! Laying a bundle out on disk: the executable in every slot, the helpers'
//! `Info.plist`s and the CEF framework, all inside the `.app`.

use std::fs;
use std::io;
use std::path::Path;

use super::{FRAMEWORK, IDENTIFIER, Layout, NAME, info_plist, passkeys};

pub fn assemble(layout: &Layout, exe: &Path, framework: &Path) -> io::Result<()> {
    let contents = layout.app.join("Contents");
    fs::create_dir_all(contents.join("MacOS"))?;
    fs::create_dir_all(contents.join("Resources"))?;
    let name = layout.executable.file_name().map_or_else(|| NAME.to_owned(), |n| n.to_string_lossy().into_owned());
    write_if_changed(&contents.join("Info.plist"), &info_plist(&name, IDENTIFIER, false))?;
    copy_executable(exe, &layout.executable)?;
    for (bundle, executable) in &layout.helpers {
        let name = executable.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        fs::create_dir_all(bundle.join("Contents/MacOS"))?;
        write_if_changed(&bundle.join("Contents/Info.plist"), &info_plist(&name, IDENTIFIER, true))?;
        copy_executable(exe, executable)?;
    }
    copy_framework(layout, framework)
}

pub(super) fn framework_stamp(framework: &Path) -> io::Result<String> {
    let binary = framework.join("Chromium Embedded Framework");
    let identity = if binary.is_file() { super::stamp(&binary)? } else { super::stamp(framework)? };
    Ok(format!("{} {identity}", framework.canonicalize()?.display()))
}

fn copy_framework(layout: &Layout, framework: &Path) -> io::Result<()> {
    let source = format!("{} {}", framework_stamp(framework)?, passkeys::ACCESS_GROUP);
    let marker = layout.app.with_extension("framework");
    let copied = fs::symlink_metadata(&layout.framework).is_ok_and(|m| m.is_dir());
    if copied && fs::read_to_string(&marker).ok().as_deref() == Some(source.as_str()) {
        return Ok(());
    }
    if fs::symlink_metadata(&layout.framework).is_ok() {
        remove(&layout.framework)?;
    }
    copy_tree(framework, &layout.framework)?;
    passkeys::patch_framework(&layout.framework)?;
    fs::write(marker, &*source)
}

pub fn assemble_release(layout: &Layout, identifier: &str) -> io::Result<()> {
    let framework = cef_dll_sys::get_cef_dir()
        .ok_or_else(|| io::Error::other("the CEF distribution was not found"))?
        .join(FRAMEWORK);
    if fs::symlink_metadata(&layout.framework).is_ok() {
        remove(&layout.framework)?;
    }
    copy_tree(&framework, &layout.framework)?;
    passkeys::patch_framework(&layout.framework)?;
    for (bundle, executable) in &layout.helpers {
        let name = executable.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        fs::create_dir_all(bundle.join("Contents/MacOS"))?;
        fs::write(bundle.join("Contents/Info.plist"), info_plist(&name, identifier, true))?;
        fs::copy(&layout.executable, executable)?;
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(&source)?, &target)?;
        } else if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else {
            fs::copy(&source, &target)?;
        }
    }
    Ok(())
}

fn write_if_changed(path: &Path, text: &str) -> io::Result<()> {
    if fs::read_to_string(path).ok().as_deref() == Some(text) { Ok(()) } else { fs::write(path, text) }
}

fn copy_executable(exe: &Path, to: &Path) -> io::Result<()> {
    if fs::symlink_metadata(to).is_ok() {
        fs::remove_file(to)?;
    }
    fs::copy(exe, to).map(|_| ())
}

fn remove(path: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembling_copies_the_binary_everywhere_and_is_repeatable() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("superfik");
        fs::write(&exe, b"v1")?;
        let framework = dir.path().join("cef").join(FRAMEWORK);
        fs::create_dir_all(&framework)?;
        let layout = Layout::new(dir.path());
        assemble(&layout, &exe, &framework)?;
        fs::remove_file(&exe)?;
        fs::write(&exe, b"v2")?;
        assemble(&layout, &exe, &framework)?;
        assert_eq!(fs::read(&layout.executable)?, b"v2");
        for (_, helper) in &layout.helpers {
            assert_eq!(fs::read(helper)?, b"v2");
        }
        assert!(!fs::symlink_metadata(&layout.framework)?.file_type().is_symlink());
        Ok(())
    }

    #[test]
    fn the_framework_is_copied_inside_the_bundle_so_its_seal_holds() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("superfik");
        fs::write(&exe, b"v1")?;
        let framework = dir.path().join("cef").join(FRAMEWORK);
        fs::create_dir_all(framework.join("Versions/A"))?;
        fs::write(framework.join("Versions/A/lib"), b"cef")?;
        std::os::unix::fs::symlink("Versions/A/lib", framework.join("lib"))?;
        let layout = Layout::new(dir.path());
        fs::create_dir_all(layout.framework.parent().ok_or_else(|| io::Error::other("no parent"))?)?;
        std::os::unix::fs::symlink(&framework, &layout.framework)?;
        assemble(&layout, &exe, &framework)?;
        assert!(fs::symlink_metadata(&layout.framework)?.is_dir());
        assert_eq!(fs::read(layout.framework.join("Versions/A/lib"))?, b"cef");
        assert_eq!(fs::read_link(layout.framework.join("lib"))?, Path::new("Versions/A/lib"));
        fs::write(layout.framework.join("Versions/A/lib"), b"signed")?;
        assert_eq!(fs::read(framework.join("Versions/A/lib"))?, b"cef");
        assemble(&layout, &exe, &framework)?;
        assert_eq!(fs::read(layout.framework.join("Versions/A/lib"))?, b"signed");
        Ok(())
    }

    #[test]
    fn the_bundled_framework_asks_for_our_passkey_group_and_the_download_is_untouched() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let exe = dir.path().join("superfik");
        fs::write(&exe, b"v1")?;
        let framework = dir.path().join("cef").join(FRAMEWORK);
        fs::create_dir_all(&framework)?;
        let stock = b"\0.org.chromium.Chromium.webauthn\0";
        fs::write(framework.join("Chromium Embedded Framework"), stock)?;
        let layout = Layout::new(dir.path());
        assemble(&layout, &exe, &framework)?;
        assert_eq!(
            fs::read(layout.framework.join("Chromium Embedded Framework"))?,
            b"\0F3BSQ3FZTF.superfik.webauthn.v1\0"
        );
        assert_eq!(fs::read(framework.join("Chromium Embedded Framework"))?, stock);
        Ok(())
    }
}
