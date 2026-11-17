use super::GitFacts;

const FALLBACK_DEFAULTS: [&str; 2] = ["main", "master"];

struct Globals {
    valued: &'static [&'static str],
    plain: &'static [&'static str],
}

const GIT: Globals = Globals {
    valued: &["-C", "-c", "--git-dir", "--work-tree", "--namespace", "--super-prefix", "--config-env", "--list-cmds"],
    plain: &[
        "-P",
        "-p",
        "--no-pager",
        "--paginate",
        "--bare",
        "--exec-path",
        "--html-path",
        "--man-path",
        "--info-path",
        "--no-replace-objects",
        "--no-lazy-fetch",
        "--no-optional-locks",
        "--no-advice",
        "--literal-pathspecs",
        "--glob-pathspecs",
        "--noglob-pathspecs",
        "--icase-pathspecs",
    ],
};

const PACKAGES: Globals = Globals {
    valued: &[
        "-C",
        "-w",
        "-F",
        "--prefix",
        "--dir",
        "--cwd",
        "--registry",
        "--userconfig",
        "--globalconfig",
        "--cache",
        "--workspace",
        "--filter",
        "--loglevel",
        "--otp",
        "--tag",
        "--access",
    ],
    plain: &[
        "-g",
        "-r",
        "-s",
        "-q",
        "-y",
        "-ws",
        "--global",
        "--recursive",
        "--workspaces",
        "--silent",
        "--quiet",
        "--yes",
    ],
};

const CARGO: Globals = Globals {
    valued: &["-C", "-Z", "--config", "--color", "--manifest-path"],
    plain: &["-q", "-v", "-vv", "--quiet", "--verbose", "--locked", "--offline", "--frozen"],
};

const GH: Globals = Globals { valued: &["-R", "--repo", "--hostname"], plain: &[] };

const CONTAINERS: Globals = Globals {
    valued: &[
        "-H",
        "-c",
        "-l",
        "--host",
        "--context",
        "--config",
        "--log-level",
        "--tlscacert",
        "--tlscert",
        "--tlskey",
    ],
    plain: &["-D", "--debug", "--tls", "--tlsverify"],
};

pub fn risk(words: &[&str], git: &GitFacts<'_>) -> Option<String> {
    let (program, arguments) = words.split_first()?;
    let each = |globals: &Globals, judged: &dyn Fn(&[&str]) -> Option<String>| {
        after_globals(arguments, globals).into_iter().find_map(judged)
    };
    match *program {
        "git" => each(&GIT, &|rest| match rest {
            ["push", rest @ ..] => push_risk(rest, git),
            _ => None,
        }),
        "cargo" => {
            let toolchain = arguments.first().is_some_and(|word| word.starts_with('+'));
            let arguments = if toolchain { &arguments[1..] } else { arguments };
            let publishes = |rest: &[&str]| (rest.first() == Some(&"publish")).then(publishing);
            after_globals(arguments, &CARGO).into_iter().find_map(publishes)
        }
        "npm" | "pnpm" | "yarn" => each(&PACKAGES, &|rest| rest.contains(&"publish").then(publishing)),
        "gh" => each(&GH, &|rest| match rest {
            ["pr", "merge", ..] => Some("it merges a pull request".into()),
            ["release", "create" | "delete" | "edit", ..] => Some("it changes a release".into()),
            ["api", ..] => Some("it calls the GitHub API directly".into()),
            _ => None,
        }),
        "docker" | "podman" => each(&CONTAINERS, &|rest| match rest {
            ["push", ..] | ["image", "push", ..] => Some("it pushes an image".into()),
            _ => None,
        }),
        _ => None,
    }
}

fn publishing() -> String {
    "it publishes a package".into()
}

fn after_globals<'a>(arguments: &'a [&'a str], globals: &Globals) -> Vec<&'a [&'a str]> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(word) = arguments.get(at) {
        if *word == "--" {
            found.push(&arguments[at + 1..]);
            return found;
        }
        if word.len() < 2 || !word.starts_with('-') {
            found.push(&arguments[at..]);
            return found;
        }
        let name = word.split_once('=').map_or(*word, |(name, _)| name);
        let attached = globals.valued.iter().any(|short| short.len() == 2 && word.len() > 2 && word.starts_with(short));
        if word.contains('=') || globals.plain.contains(&name) || (attached && !word.starts_with("--")) {
            at += 1;
        } else if globals.valued.contains(&name) {
            at += 2;
        } else {
            found.push(arguments.get(at + 1..).unwrap_or_default());
            at += 2;
        }
    }
    found
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
