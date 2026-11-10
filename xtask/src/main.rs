//! Developer tasks: `lint` (repo rules clippy cannot check).

mod lint;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task] if task == "lint" => lint::run(&root(), false),
        [task, flag] if task == "lint" && flag == "--update-baseline" => lint::run(&root(), true),
        _ => Err("usage: xtask lint [--update-baseline]".into()),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
