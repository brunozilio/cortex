//! A windowed Windows executable has no console; keep its diagnostics in
//! LocalAppData. Other desktops retain their terminal/journal output.

use std::io::{self, Write};
use std::sync::Mutex;

pub fn writer() -> impl for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static {
    let output: Box<dyn Write + Send> = output().unwrap_or_else(|error| {
        eprintln!("could not open Superfik's log: {error}");
        Box::new(io::stderr())
    });
    Mutex::new(output)
}

#[cfg(target_os = "windows")]
fn output() -> io::Result<Box<dyn Write + Send>> {
    let root = superfik_backend::local_data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is unavailable"))?
        .join("logs");
    std::fs::create_dir_all(&root)?;
    let name = format!("superfik-{}-{}.log", chrono::Local::now().format("%Y-%m-%d"), std::process::id());
    Ok(Box::new(std::fs::OpenOptions::new().create(true).append(true).open(root.join(name))?))
}

#[cfg(not(target_os = "windows"))]
fn output() -> io::Result<Box<dyn Write + Send>> {
    Ok(Box::new(io::stderr()))
}
