use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

type Failure = Box<dyn std::error::Error>;

const BUN: &str = "bun";
const SOURCES: [&str; 9] = [
    "src",
    "docs",
    "public",
    "index.html",
    "package.json",
    "bun.lock",
    "vite.config.ts",
    "tailwind.config.js",
    "tsconfig.app.json",
];
const FIXTURES: &str = "web_fixtures";
const UPDATE: &str = "UPDATE_WEB_FIXTURES";

pub fn build(root: &Path) -> Result<(), Failure> {
    let web = root.join("web");
    built_with(BUN, &web)?;
    println!("web: built into {}", web.join("dist").display());
    Ok(())
}

fn built_with(program: &str, web: &Path) -> Result<(), Failure> {
    if !web.join("package.json").is_file() {
        return Err(format!("{} has no package.json", web.display()).into());
    }
    run(program, web, &["install", "--frozen-lockfile"])?;
    run(program, web, &["run", "build"])?;
    if web.join("dist/index.html").is_file() {
        Ok(())
    } else {
        Err(format!("`{program} run build` succeeded but wrote no {}", web.join("dist/index.html").display()).into())
    }
}

fn run(program: &str, folder: &Path, arguments: &[&str]) -> Result<(), Failure> {
    let asked = format!("{program} {}", arguments.join(" "));
    let status = Command::new(program).args(arguments).current_dir(folder).status().map_err(|error| {
        format!("could not run `{asked}` ({error}); the page is built with Bun, install it from https://bun.sh")
    })?;
    if status.success() { Ok(()) } else { Err(format!("`{asked}` failed in {} ({status})", folder.display()).into()) }
}

pub fn check_release(root: &Path) -> Result<(), Failure> {
    match release_problem(&root.join("web"))? {
        None => {
            println!("check-release: web/dist is newer than the page's sources");
            Ok(())
        }
        Some(problem) => Err(format!("{problem}; run `cargo run -p xtask -- web`, then build the release").into()),
    }
}

fn release_problem(web: &Path) -> Result<Option<String>, Failure> {
    let index = web.join("dist/index.html");
    let Ok(built) = std::fs::metadata(&index).and_then(|found| found.modified()) else {
        return Ok(Some(format!("{} is missing, so the release would carry no page", index.display())));
    };
    let mut newest: Option<(SystemTime, PathBuf)> = None;
    for source in SOURCES {
        latest(&web.join(source), &mut newest)?;
    }
    Ok(newest.filter(|(changed, _)| *changed > built).map(|(_, path)| {
        format!("{} is older than {}, so the release would carry a stale page", index.display(), path.display())
    }))
}

fn latest(path: &Path, newest: &mut Option<(SystemTime, PathBuf)>) -> Result<(), Failure> {
    let Ok(found) = std::fs::metadata(path) else { return Ok(()) };
    if found.is_dir() {
        for entry in std::fs::read_dir(path)? {
            latest(&entry?.path(), newest)?;
        }
        return Ok(());
    }
    let changed = found.modified()?;
    if newest.as_ref().is_none_or(|(known, _)| changed > *known) {
        *newest = Some((changed, path.to_owned()));
    }
    Ok(())
}

pub fn fixtures() -> Result<(), Failure> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status =
        Command::new(&cargo).args(["test", "-q", "-p", "xtask", "--test", FIXTURES]).env(UPDATE, "1").status()?;
    if status.success() {
        println!("web-fixtures: web/test/fixtures written; run `bun test` in web/ next");
        Ok(())
    } else {
        Err(format!("`{cargo} test -p xtask --test {FIXTURES}` failed ({status})").into())
    }
}

pub fn schemas() -> Result<(), Failure> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let test = "config_schema::tests::the_published_schemas_are_what_the_loaders_check";
    let status = Command::new(&cargo)
        .args(["test", "-q", "-p", "agent-engine", "--lib", "--", "--exact", test])
        .env("UPDATE_SCHEMAS", "1")
        .status()?;
    if status.success() {
        println!("schemas: content/schemas written");
        Ok(())
    } else {
        Err(format!("`{cargo} test -p agent-engine --lib {test}` failed ({status})").into())
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
