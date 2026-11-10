//! Windows' MSI install: finding it, staging the MSI after its version is
//! read, and the swap handed to PowerShell.

use std::ffi::OsString;
use std::path::Path;

use super::Tools;
use crate::*;

fn msi(dir: &Path, tools: &Tools) -> Msi {
    let exe = dir.join("Program Files/Superfik/superfik.exe");
    Msi::new(exe, dir.join("staging"), Box::new(tools.clone()), 42, dir.join("scripts"))
}

fn download(dir: &Path) -> std::path::PathBuf {
    let archive = dir.join("superfik-0.15.0-windows-x86_64.download");
    std::fs::write(&archive, "msi").expect("archive");
    archive
}

#[test]
fn only_the_installed_superfik_exe_is_an_msi_install() {
    let installed = Path::new("C:/Program Files/Superfik/superfik.exe");
    assert_eq!(msi_install_of(installed).as_deref(), Some(installed));
    assert!(msi_install_of(Path::new("C:/Program Files/SUPERFIK/Superfik.EXE")).is_some());
    assert_eq!(msi_install_of(Path::new("C:/src/superfik/target/release/superfik.exe")), None);
    assert_eq!(msi_install_of(Path::new("C:/Program Files/Superfik/other.exe")), None);
}

#[test]
fn the_version_query_quotes_the_path_for_powershell() {
    let query = version_query(Path::new("C:/Users/o'neil/Superfik-0.15.0-x64.msi"));
    assert!(query.contains("@('C:/Users/o''neil/Superfik-0.15.0-x64.msi', 0)"));
    assert!(query.contains("Property='ProductVersion'"));
}

#[test]
fn an_msi_of_the_offered_version_is_staged_and_swapped_in_by_powershell() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tools = Tools::answering(&[(true, "0.15.0\r\n")]);
    let installer = msi(dir.path(), &tools);
    let prepared = installer.stage(&download(dir.path()), &Version::new(0, 15, 0)).expect("staged");
    assert_eq!(prepared.staged, dir.path().join("staging/Superfik-0.15.0-x64.msi"));
    assert_eq!(std::fs::read_to_string(&prepared.staged).expect("staged msi"), "msi");
    installer.restart(&prepared).expect("restart");
    let calls = tools.calls.lock().expect("lock");
    let (program, args) = &calls[1];
    assert_eq!(program, "powershell.exe");
    let script = args.iter().position(|arg| arg == "-File").map(|at| Path::new(&args[at + 1]).to_path_buf());
    let script = script.expect("a script");
    assert!(script.starts_with(dir.path().join("scripts")));
    assert_eq!(std::fs::read_to_string(&script).expect("script"), MSI_SWAP_SCRIPT);
    let tail: Vec<OsString> = vec![
        "42".into(),
        prepared.staged.clone().into(),
        dir.path().join("Program Files/Superfik/superfik.exe").into(),
    ];
    assert_eq!(args[args.len() - 3..], tail[..]);
}

#[test]
fn an_msi_of_another_version_is_refused_and_removed() {
    let dir = tempfile::tempdir().expect("temp dir");
    for answer in [(true, "0.14.0"), (false, "Exception calling OpenDatabase")] {
        let tools = Tools::answering(&[answer]);
        let staged = msi(dir.path(), &tools).stage(&download(dir.path()), &Version::new(0, 15, 0));
        assert!(matches!(staged, Err(UpdateError::Install(_))), "{staged:?}");
        assert!(!dir.path().join("staging").exists());
    }
}

#[test]
fn restart_refuses_an_msi_that_was_not_staged() {
    let dir = tempfile::tempdir().expect("temp dir");
    let tools = Tools::default();
    let elsewhere = download(dir.path());
    let prepared = Prepared { version: Version::new(0, 15, 0), staged: elsewhere };
    assert!(msi(dir.path(), &tools).restart(&prepared).is_err());
    assert!(tools.calls.lock().expect("lock").is_empty());
}

#[cfg(windows)]
#[test]
fn the_swap_script_parses_in_windows_powershell() {
    let dir = tempfile::tempdir().expect("temp dir");
    let script = dir.path().join("swap.ps1");
    std::fs::write(&script, MSI_SWAP_SCRIPT).expect("script");
    let check = format!(
        "$e = $null; [void][System.Management.Automation.Language.Parser]::ParseFile('{}', [ref]$null, [ref]$e); \
         if ($e.Count) {{ $e | ForEach-Object {{ $_.Message }}; exit 1 }}",
        script.display()
    );
    let ran =
        System.run("powershell.exe", &["-NoProfile".into(), "-Command".into(), check.into()]).expect("powershell");
    assert!(ran.success, "{}", ran.output);
}

#[cfg(windows)]
#[test]
#[ignore = "needs a built MSI: SUPERFIK_TEST_MSI and SUPERFIK_TEST_MSI_VERSION"]
fn reads_a_built_msis_version() {
    let msi = std::env::var("SUPERFIK_TEST_MSI").expect("SUPERFIK_TEST_MSI");
    let version = std::env::var("SUPERFIK_TEST_MSI_VERSION").expect("SUPERFIK_TEST_MSI_VERSION");
    let query = version_query(&std::fs::canonicalize(msi).expect("msi"));
    let ran =
        System.run("powershell.exe", &["-NoProfile".into(), "-Command".into(), query.into()]).expect("powershell");
    assert!(ran.success, "{}", ran.output);
    assert_eq!(ran.output.trim(), version);
}
