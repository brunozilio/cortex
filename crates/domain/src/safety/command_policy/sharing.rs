use super::GitFacts;

const FALLBACK_DEFAULTS: [&str; 2] = ["main", "master"];

pub fn risk(words: &[&str], git: &GitFacts<'_>) -> Option<String> {
    match words {
        ["git", "push", rest @ ..] => push_risk(rest, git),
        ["cargo" | "npm" | "pnpm" | "yarn", "publish", ..] => Some("it publishes a package".into()),
        ["gh", "pr", "merge", ..] => Some("it merges a pull request".into()),
        ["gh", "release", "create" | "delete" | "edit", ..] => Some("it changes a release".into()),
        ["gh", "api", ..] => Some("it calls the GitHub API directly".into()),
        ["docker" | "podman", "push", ..] => Some("it pushes an image".into()),
        _ => None,
    }
}

fn push_risk(args: &[&str], git: &GitFacts<'_>) -> Option<String> {
    let forced = args.iter().any(|arg| {
        matches!(*arg, "-f" | "--force" | "--mirror" | "--delete" | "-d" | "--prune")
            || arg.starts_with("--force-with-lease")
            || arg.starts_with("--force-if-includes")
    });
    let refspecs: Vec<&str> = args.iter().filter(|arg| !arg.starts_with('-')).skip(1).copied().collect();
    if forced || refspecs.iter().any(|spec| spec.starts_with('+') || spec.starts_with(':')) {
        return Some("it rewrites or deletes history on the remote".into());
    }
    let targets: Vec<&str> = if refspecs.is_empty() {
        git.current_branch.into_iter().collect()
    } else {
        refspecs.iter().map(|spec| spec.rsplit(':').next().unwrap_or(spec)).collect()
    };
    let default = |name: &str| {
        let name = name.trim_start_matches("refs/heads/");
        git.default_branch.map_or(FALLBACK_DEFAULTS.contains(&name), |default| default == name)
    };
    targets.into_iter().any(default).then(|| "it pushes to the default branch".into())
}
