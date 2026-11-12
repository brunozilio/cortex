use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use domain::lsp::install::{Archive, Install, Recipe};
use platform::{Bounds, Ended};

use super::Starter;
use super::installs::Installer;
use crate::model::http::{Http, NetHttp};

const TIMEOUT: Duration = Duration::from_secs(10 * 60);
const OUTPUT: usize = 64 * 1024;

#[derive(Debug, Default)]
pub struct Downloads;

impl Installer for Downloads {
    fn install(&self, install: &Install, home: &Path) -> Result<(), String> {
        settle(home, |staging| fill(install, staging).and_then(|()| executable(&staging.join(&install.program))))
    }
}

fn settle(home: &Path, fill: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let parent = home.parent().ok_or("the install folder has no parent")?;
    let name = home.file_name().ok_or("the install folder has no name")?.to_string_lossy();
    let staging = parent.join(format!(".{name}.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|error| format!("cannot create {}: {error}", staging.display()))?;
    if let Err(error) = fill(&staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    let _ = std::fs::remove_dir_all(home);
    std::fs::rename(&staging, home).map_err(|error| format!("cannot move the install into place: {error}"))
}

fn fill(install: &Install, staging: &Path) -> Result<(), String> {
    match &install.recipe {
        Recipe::Npm { packages } => {
            let mut command = Command::new(tool("npm", "Node.js")?);
            command.arg("install").arg("--prefix").arg(staging).args(["--no-audit", "--no-fund", "--loglevel=error"]);
            run(command.args(packages))
        }
        Recipe::Go { module } => {
            let mut command = Command::new(tool("go", "Go")?);
            run(command.arg("install").arg(module).env("GOBIN", staging.join("bin")))
        }
        Recipe::Download { url, archive } => download(url, *archive, staging, &staging.join(&install.program)),
    }
}

fn tool(name: &str, runtime: &str) -> Result<PathBuf, String> {
    super::Processes.find(name).ok_or_else(|| format!("it needs `{name}` on PATH; install {runtime} first."))
}

fn run(command: &mut Command) -> Result<(), String> {
    let never = AtomicBool::new(false);
    let finished =
        platform::bounded::run(command, Bounds { timeout: TIMEOUT, max_output: OUTPUT, stop: &never, input: None })
            .map_err(|error| error.to_string())?;
    match finished.ended {
        Ended::Exited(status) if status.success() => Ok(()),
        Ended::Exited(status) => Err(format!("{status}: {}", last_lines(&finished.stderr))),
        Ended::TimedOut => Err("it took longer than 10 minutes".into()),
        Ended::Stopped => Err("it was stopped".into()),
    }
}

fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|line| !line.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(5)..].join("\n")
}

fn download(url: &str, archive: Archive, staging: &Path, program: &Path) -> Result<(), String> {
    let never = AtomicBool::new(false);
    let response = NetHttp.get_stream(url, &[], &never).map_err(|error| error.to_string())?;
    if response.status != 200 {
        return Err(format!("{url} answered {}", response.status));
    }
    unpack(Patient::new(response.body, QUIET), archive, staging, program).map_err(|error| format!("{url}: {error}"))
}

fn unpack(mut body: impl io::Read, archive: Archive, staging: &Path, program: &Path) -> Result<(), String> {
    let saved = |error: io::Error| format!("cannot save the download: {error}");
    match archive {
        Archive::Gzip => {
            let mut file = File::create(program).map_err(saved)?;
            io::copy(&mut flate2::read::GzDecoder::new(body), &mut file).map_err(saved)?;
        }
        Archive::Zip => {
            let path = staging.join(".download.zip");
            io::copy(&mut body, &mut File::create(&path).map_err(saved)?).map_err(saved)?;
            let file = File::open(&path).map_err(saved)?;
            let unpacked = zip::ZipArchive::new(file)
                .map_err(|error| format!("not a zip: {error}"))
                .and_then(|mut zip| zip.extract(staging).map_err(|error| format!("cannot unpack: {error}")));
            std::fs::remove_file(&path).map_err(saved)?;
            unpacked?;
        }
    }
    Ok(())
}

const QUIET: Duration = Duration::from_secs(60);

struct Patient<R> {
    inner: R,
    quiet: Duration,
}

impl<R: io::Read> Patient<R> {
    const fn new(inner: R, quiet: Duration) -> Self {
        Self { inner, quiet }
    }
}

impl<R: io::Read> io::Read for Patient<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let started = std::time::Instant::now();
        loop {
            match self.inner.read(buf) {
                Err(error) if error.kind() == io::ErrorKind::TimedOut && started.elapsed() < self.quiet => {}
                other => return other,
            }
        }
    }
}

fn executable(program: &Path) -> Result<(), String> {
    let missing = |error: io::Error| format!("{} is missing after installing: {error}", program.display());
    let metadata = std::fs::metadata(program).map_err(missing)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = metadata.permissions();
        permissions.set_mode(permissions.mode() | 0o755);
        std::fs::set_permissions(program, permissions).map_err(missing)?;
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::Read;

    use super::*;

    struct Stalling(VecDeque<io::Result<Vec<u8>>>);

    impl Read for Stalling {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.0.pop_front() {
                Some(Ok(bytes)) => {
                    buf[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Some(Err(error)) => Err(error),
                None => Ok(0),
            }
        }
    }

    fn pause() -> io::Result<Vec<u8>> {
        Err(io::Error::new(io::ErrorKind::TimedOut, "waiting for stream"))
    }

    #[test]
    fn an_install_lands_whole_or_not_at_all() {
        let data = tempfile::tempdir().expect("data");
        let home = data.path().join("gopls");
        std::fs::create_dir_all(&home).expect("home");
        std::fs::write(home.join("old"), "").expect("old");
        let broken = settle(&home, |staging| {
            std::fs::write(staging.join("half"), "").map_err(|e| e.to_string())?;
            Err("go: exit status 1".into())
        });
        assert_eq!(broken, Err("go: exit status 1".into()));
        assert!(home.join("old").is_file());
        assert_eq!(std::fs::read_dir(data.path()).expect("list").count(), 1);
        settle(&home, |staging| std::fs::write(staging.join("new"), "").map_err(|e| e.to_string())).expect("settled");
        assert!(home.join("new").is_file() && !home.join("old").exists());
        assert_eq!(std::fs::read_dir(data.path()).expect("list").count(), 1);
    }

    #[test]
    fn a_gzip_is_the_program_and_a_zip_unpacks_into_the_folder() {
        let staging = tempfile::tempdir().expect("staging");
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        io::Write::write_all(&mut gzip, b"#!bin").expect("gzip");
        let program = staging.path().join("rust-analyzer");
        unpack(&gzip.finish().expect("gzip")[..], Archive::Gzip, staging.path(), &program).expect("gunzip");
        assert_eq!(std::fs::read(&program).expect("program"), b"#!bin");
        let mut zip = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        zip.start_file("clangd_1/bin/clangd", zip::write::SimpleFileOptions::default()).expect("entry");
        io::Write::write_all(&mut zip, b"elf").expect("zip");
        let bytes = zip.finish().expect("zip").into_inner();
        let program = staging.path().join("clangd_1/bin/clangd");
        unpack(&bytes[..], Archive::Zip, staging.path(), &program).expect("unzip");
        assert_eq!(std::fs::read(&program).expect("program"), b"elf");
        assert!(!staging.path().join(".download.zip").exists());
        assert!(unpack(&b"not a zip"[..], Archive::Zip, staging.path(), &program).is_err());
    }

    #[test]
    fn a_failed_command_reports_its_last_lines() {
        assert_eq!(last_lines("a\n\nb\nc\n  \nd\ne\nf\n"), "b\nc\nd\ne\nf");
    }

    #[test]
    fn a_download_waits_through_quiet_moments_but_not_forever() {
        let mut read = Vec::new();
        let stalling = Stalling([pause(), Ok(b"ab".to_vec()), pause(), Ok(b"c".to_vec())].into());
        Patient::new(stalling, Duration::from_secs(5)).read_to_end(&mut read).expect("read");
        assert_eq!(read, b"abc");
        let silent = Stalling(std::iter::repeat_with(pause).take(1000).collect());
        let error = Patient::new(silent, Duration::ZERO).read_to_end(&mut Vec::new()).expect_err("gave up");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
