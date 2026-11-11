mod brand;
mod comments;
mod lint;

use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task] if task == "lint" => lint::run(&root(), false),
        [task] if task == "brand" => brand::run(&root()),
        [task, files @ ..] if task == "comments" => comments::check(files),
        [task, flag] if task == "lint" && flag == "--update-baseline" => lint::run(&root(), true),
        _ => Err("usage: xtask lint [--update-baseline] | xtask brand | xtask comments <file>…".into()),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
