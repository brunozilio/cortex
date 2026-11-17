use std::path::Path;

use domain::changes::{Changes, DiffScope};

pub fn changes_in(cwd: &Path, scope: DiffScope) -> Result<Changes, String> {
    const DIFF: [&str; 4] = ["diff", "--no-color", "--no-ext-diff", "--no-textconv"];
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

pub fn inside(cwd: &Path) -> bool {
    git(cwd, &["rev-parse", "--git-dir"]).is_ok()
}

pub fn init(cwd: &Path) -> Result<bool, String> {
    git(cwd, &["init", "--quiet"])?;
    let has_commit = git(cwd, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let first =
        ["-c", "commit.gpgsign=false", "commit", "--quiet", "--allow-empty", "--no-verify", "-m", "Initial commit"];
    Ok(!has_commit && git(cwd, &first).is_ok())
}

fn staged(cwd: &Path) -> Result<String, String> {
    git(cwd, &["diff", "--cached", "--no-color", "--no-ext-diff", "--no-textconv"])
}

fn branch_base(cwd: &Path) -> Result<String, String> {
    ["origin/HEAD", "main", "master", "@{upstream}"]
        .iter()
        .find_map(|base| git(cwd, &["merge-base", "HEAD", base]).ok())
        .map(|commit| commit.trim().to_owned())
        .ok_or_else(|| "no main, master or upstream branch to compare with".to_owned())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    crate::git::run(cwd, &[&["-c", "core.quotepath=off"], args].concat(), crate::git::LOCAL)
}
