//! Offline maintenance utility. Never invokes a model or a provider CLI.
use std::path::PathBuf;
fn run() -> Result<(), String> {
    let mut manifest = None;
    let mut root = None;
    let mut destination = None;
    let mut apply = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => manifest = Some(PathBuf::from(args.next().ok_or("missing manifest")?)),
            "--source-root" => root = Some(PathBuf::from(args.next().ok_or("missing source root")?)),
            "--sessions-dir" => destination = Some(PathBuf::from(args.next().ok_or("missing sessions directory")?)),
            "--apply" => apply = true,
            "--help" => {
                println!(
                    "superfik-session-migrate --manifest FILE --source-root PRIVATE_EXPORT_DIR --sessions-dir NATIVE_SESSIONS_DIR [--apply]\nDefault: validate only. Run as the server data owner. No model/CLI inference is started."
                );
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }
    let prepared = superfik_native::sessions::migration::prepare(
        &manifest.ok_or("--manifest is required")?,
        &root.ok_or("--source-root is required")?,
    )?;
    let destination = destination.ok_or("--sessions-dir is required")?;
    if apply {
        let report = prepared.apply(&superfik_native::sessions::FileSessions::new(destination))?;
        println!("{}", serde_json::to_string(&report).map_err(|e| e.to_string())?);
    } else {
        println!("validated_entries={}; applied=false", prepared.count());
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("session migration: {error}");
        std::process::exit(1);
    }
}
