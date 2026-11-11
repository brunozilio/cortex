//! Developer tasks: `lint` (repo rules clippy cannot check) and `brand`
//! (applies brand.toml to the files that spell it out).

mod brand;
mod lint;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task] if task == "lint" => lint::run(&root(), false),
        [task] if task == "brand" => brand::run(&root()),
        [task, flag] if task == "lint" && flag == "--update-baseline" => lint::run(&root(), true),
        _ => Err("usage: xtask lint [--update-baseline] | xtask brand".into()),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
