mod brand;
mod comments;
mod crate_graph;
mod crate_table;
mod drift;
mod env_vars;
mod git_commands;
mod lint;
mod web;

use std::path::PathBuf;

const USAGE: &str = "usage: xtask lint [--update-baseline] | xtask brand | xtask comments <file>… | xtask web | \
                     xtask check-release | xtask web-fixtures | xtask schemas";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task] if task == "lint" => lint::run(&root(), false),
        [task] if task == "brand" => brand::run(&root()),
        [task, files @ ..] if task == "comments" => comments::check(files),
        [task, flag] if task == "lint" && flag == "--update-baseline" => lint::run(&root(), true),
        [task] if task == "web" => web::build(&root()),
        [task] if task == "check-release" => web::check_release(&root()),
        [task] if task == "web-fixtures" => web::fixtures(),
        [task] if task == "schemas" => web::schemas(),
        _ => Err(USAGE.into()),
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
