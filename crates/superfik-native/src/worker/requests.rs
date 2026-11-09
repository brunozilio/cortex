//! Typed requests sent to the native worker.

use super::*;

pub fn brief(
    program: &std::path::Path,
    request: &superfik_core::PromptRequest,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<crate::brief::Brief, String> {
    use std::io::Write;
    let mut command = superfik_process::command(program);
    clean_env(&mut command);
    command.arg("--brief").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    let mut tree = superfik_process::Tree::spawn(&mut command).map_err(|e| e.to_string())?;
    let mut input = tree.child().stdin.take().ok_or("worker input missing")?;
    serde_json::to_writer(&mut input, request).map_err(|e| e.to_string())?;
    input.write_all(b"\n").and_then(|_| input.flush()).map_err(|e| e.to_string())?;
    let output = tree.child().stdout.take().ok_or("worker output missing")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output.take(64 * 1024 * 1024).read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    loop {
        if let Some(status) = tree.child().try_wait().map_err(|e| e.to_string())? {
            let bytes = reader.join().map_err(|_| "worker reader stopped")?.map_err(|e| e.to_string())?;
            if !status.success() {
                return Err("isolated context worker could not read the requested context".into());
            }
            return serde_json::from_slice(&bytes).map_err(|e| e.to_string());
        }
        if stop.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(30) {
            tree.kill();
            let _ = reader.join();
            return Err("context preparation cancelled or timed out".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
pub fn read_text(program: &std::path::Path, path: &std::path::Path, workspace: &Workspace) -> Result<String, String> {
    let outcome = run(program, "ReadSkill", &serde_json::json!({"file_path":path}), workspace)?;
    if outcome.is_error { Err(outcome.content) } else { Ok(outcome.content) }
}

pub fn operation<T: serde::de::DeserializeOwned>(
    program: &std::path::Path,
    name: &str,
    input: &Value,
    workspace: &Workspace,
) -> Result<T, String> {
    let result = run(program, name, input, workspace)?;
    if result.is_error {
        return Err(result.content);
    }
    serde_json::from_str(&result.content).map_err(|e| e.to_string())
}
