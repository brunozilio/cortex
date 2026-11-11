use std::path::Path;
use std::process::Command;

use domain::changes::{Changes, DiffScope};

pub fn changes_in(cwd: &Path, scope: DiffScope) -> Result<Changes, String> {
    const DIFF: [&str; 3] = ["diff", "--no-color", "--no-ext-diff"];
    let diff = match scope {
        DiffScope::Uncommitted => git(cwd, &[&DIFF[..], &["HEAD"]].concat()).or_else(|_| staged(cwd))?,
        DiffScope::Unstaged => git(cwd, &DIFF)?,
        DiffScope::Staged => staged(cwd)?,
        DiffScope::Branch => git(cwd, &[&DIFF[..], &[branch_base(cwd)?.as_str()]].concat())?,
    };
    let untracked = match scope {
        DiffScope::Staged => String::new(),
        _ => git(cwd, &["ls-files", "--others", "--exclude-standard"])?,
    };
    Ok(Changes { diff, untracked: untracked.lines().map(str::to_owned).collect() })
}

pub fn files(cwd: &Path) -> Vec<String> {
    git(cwd, &["ls-files", "--cached", "--others", "--exclude-standard"])
        .map(|listed| listed.lines().take(50_000).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn staged(cwd: &Path) -> Result<String, String> {
    git(cwd, &["diff", "--cached", "--no-color", "--no-ext-diff"])
}

fn branch_base(cwd: &Path) -> Result<String, String> {
    ["origin/HEAD", "main", "master", "@{upstream}"]
        .iter()
        .find_map(|base| git(cwd, &["merge-base", "HEAD", base]).ok())
        .map(|commit| commit.trim().to_owned())
        .ok_or_else(|| "no main, master or upstream branch to compare with".to_owned())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-c", "core.quotepath=off"])
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_owned())
    }
}
