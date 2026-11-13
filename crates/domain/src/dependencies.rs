use crate::permission_rules::{normalized, split_command};

const NEW_DAYS: i64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecosystem {
    Npm,
    Crates,
    PyPi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub ecosystem: Ecosystem,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Found {
    Missing,
    Published { first_day: Option<i64> },
    Unknown,
}

impl Ecosystem {
    pub fn lookup_url(self, name: &str) -> String {
        match self {
            Self::Npm => format!("https://registry.npmjs.org/{}", name.replace('/', "%2F")),
            Self::Crates => format!("https://crates.io/api/v1/crates/{name}"),
            Self::PyPi => format!("https://pypi.org/pypi/{name}/json"),
        }
    }

    pub fn first_published(self, answer: &serde_json::Value) -> Option<&str> {
        match self {
            Self::Npm => answer["time"]["created"].as_str(),
            Self::Crates => answer["crate"]["created_at"].as_str(),
            Self::PyPi => answer["releases"]
                .as_object()?
                .values()
                .filter_map(|files| files[0]["upload_time_iso_8601"].as_str())
                .min(),
        }
    }

    fn popular(self) -> &'static [&'static str] {
        match self {
            Self::Npm => &[
                "react",
                "lodash",
                "express",
                "axios",
                "chalk",
                "commander",
                "debug",
                "moment",
                "request",
                "typescript",
                "webpack",
                "eslint",
                "prettier",
                "jest",
                "vue",
                "next",
                "dotenv",
                "uuid",
                "yargs",
                "colors",
            ],
            Self::Crates => &[
                "serde",
                "tokio",
                "rand",
                "regex",
                "clap",
                "anyhow",
                "thiserror",
                "reqwest",
                "chrono",
                "log",
                "syn",
                "quote",
                "futures",
                "hyper",
                "tracing",
                "base64",
                "libc",
                "itertools",
                "bytes",
                "uuid",
            ],
            Self::PyPi => &[
                "requests",
                "numpy",
                "pandas",
                "django",
                "flask",
                "urllib3",
                "boto3",
                "setuptools",
                "pyyaml",
                "pytest",
                "scipy",
                "matplotlib",
                "pillow",
                "click",
                "jinja2",
                "colorama",
                "certifi",
                "openai",
            ],
        }
    }
}

pub fn requested(command: &str) -> Vec<Package> {
    let (parts, _) = split_command(command);
    parts.into_iter().flat_map(|part| in_part(&normalized(part))).collect()
}

fn in_part(run: &str) -> Vec<Package> {
    let words: Vec<&str> = run.split(' ').filter(|word| !word.is_empty()).collect();
    let (ecosystem, names) = match words.as_slice() {
        ["npm" | "pnpm" | "bun", "add" | "install" | "i", rest @ ..] | ["yarn", "add", rest @ ..] => {
            (Ecosystem::Npm, rest)
        }
        ["cargo", "add", rest @ ..] => (Ecosystem::Crates, rest),
        ["pip" | "pip3" | "pipx", "install", rest @ ..]
        | ["uv", "pip", "install", rest @ ..]
        | ["uv" | "poetry", "add", rest @ ..]
        | ["python" | "python3", "-m", "pip", "install", rest @ ..] => (Ecosystem::PyPi, rest),
        _ => return Vec::new(),
    };
    let mut skip_next = false;
    names
        .iter()
        .filter(|word| {
            let skip = std::mem::take(&mut skip_next);
            skip_next = matches!(
                **word,
                "-r" | "--requirement"
                    | "-e"
                    | "--editable"
                    | "--path"
                    | "--git"
                    | "-c"
                    | "-F"
                    | "--features"
                    | "--rename"
                    | "--branch"
                    | "--tag"
                    | "--rev"
                    | "--registry"
                    | "-p"
                    | "--package"
            );
            !skip && !word.starts_with('-')
        })
        .filter_map(|word| bare(ecosystem, word))
        .map(|name| Package { ecosystem, name })
        .collect()
}

fn bare(ecosystem: Ecosystem, word: &str) -> Option<String> {
    let word = word.trim_matches(|c| c == '"' || c == '\'');
    if word.contains("://") || word.starts_with('.') || word.starts_with('/') || word.ends_with(".txt") {
        return None;
    }
    let name = match ecosystem {
        Ecosystem::Npm => match word.strip_prefix('@') {
            Some(scoped) => format!("@{}", scoped.split('@').next().unwrap_or_default()),
            None => word.split('@').next().unwrap_or_default().to_owned(),
        },
        Ecosystem::Crates => word.split('@').next().unwrap_or_default().to_owned(),
        Ecosystem::PyPi => word.split(['=', '<', '>', '~', '!', '[', ';']).next().unwrap_or_default().to_owned(),
    };
    (!name.is_empty() && !name.contains(':')).then_some(name)
}

pub fn concern(package: &Package, found: &Found, today: i64) -> Option<String> {
    let name = &package.name;
    let lookalike = package.ecosystem.popular().iter().find(|known| **known != name && close(known, name));
    match (found, lookalike) {
        (Found::Missing, _) => Some(format!("{name} is not published in its registry: the name may be made up")),
        (_, Some(known)) => Some(format!("{name} looks like the popular {known} with a typo")),
        (Found::Published { first_day: Some(day) }, None) if today - day < NEW_DAYS => {
            Some(format!("{name} was first published {} days ago", today - day))
        }
        _ => None,
    }
}

pub fn day_of(iso: &str) -> Option<i64> {
    let year: i64 = iso.get(0..4)?.parse().ok()?;
    let month: i64 = iso.get(5..7)?.parse().ok()?;
    let day: i64 = iso.get(8..10)?.parse().ok()?;
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_era_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    Some(era * 146_097 + of_era_days - 719_468)
}

fn close(known: &str, name: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (known.chars().collect(), name.to_ascii_lowercase().chars().collect());
    if a.len().abs_diff(b.len()) > 1 || a.len() < 4 {
        return false;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let replaced = previous + usize::from(ca != cb);
            previous = row[j + 1];
            row[j + 1] = replaced.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    let swapped = a.len() == b.len() && {
        let differ: Vec<usize> = (0..a.len()).filter(|&i| a[i] != b[i]).collect();
        differ.len() == 2 && differ[1] == differ[0] + 1 && a[differ[0]] == b[differ[1]] && a[differ[1]] == b[differ[0]]
    };
    row[b.len()] == 1 || swapped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(command: &str) -> Vec<(Ecosystem, String)> {
        requested(command).into_iter().map(|package| (package.ecosystem, package.name)).collect()
    }

    #[test]
    fn installs_name_the_packages_they_fetch() {
        assert_eq!(
            names("npm install --save-dev typescript@5 @types/node@20 ./local"),
            [(Ecosystem::Npm, "typescript".into()), (Ecosystem::Npm, "@types/node".into())]
        );
        assert_eq!(names("cd app && cargo add serde@1 --features derive"), [(Ecosystem::Crates, "serde".into())]);
        assert_eq!(
            names("pip install -r requirements.txt 'requests>=2' numpy==1.26"),
            [(Ecosystem::PyPi, "requests".into()), (Ecosystem::PyPi, "numpy".into())]
        );
        assert_eq!(names("uv add httpx[http2]"), [(Ecosystem::PyPi, "httpx".into())]);
        assert!(names("npm install").is_empty());
        assert!(names("npm run build && cargo build").is_empty());
    }

    #[test]
    fn a_missing_new_or_lookalike_package_is_a_concern() {
        let package = |name: &str| Package { ecosystem: Ecosystem::Npm, name: name.into() };
        let today = day_of("2026-11-13").expect("day");
        let old = Found::Published { first_day: day_of("2015-01-01") };

        assert!(concern(&package("left-padder-x"), &Found::Missing, today).is_some_and(|why| why.contains("made up")));
        assert!(concern(&package("expresss"), &old, today).is_some_and(|why| why.contains("popular express")));
        assert!(concern(&package("axois"), &old, today).is_some_and(|why| why.contains("axios")));
        let fresh = Found::Published { first_day: day_of("2026-11-01") };
        assert_eq!(
            concern(&package("brand-new-lib"), &fresh, today),
            Some("brand-new-lib was first published 12 days ago".into())
        );
        assert_eq!(concern(&package("express"), &old, today), None);
        assert_eq!(concern(&package("left-pad"), &Found::Unknown, today), None);
    }

    #[test]
    fn dates_count_whole_days_and_registries_say_when_a_package_began() {
        assert_eq!(day_of("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(day_of("2000-03-01"), Some(11_017));
        assert_eq!(day_of("nonsense"), None);
        let npm = serde_json::json!({ "time": { "created": "2011-06-01T00:00:00Z" } });
        assert_eq!(Ecosystem::Npm.first_published(&npm), Some("2011-06-01T00:00:00Z"));
        let pypi = serde_json::json!({ "releases": {
            "2.0": [{ "upload_time_iso_8601": "2020-01-01T00:00:00Z" }],
            "1.0": [{ "upload_time_iso_8601": "2012-01-01T00:00:00Z" }],
            "0.0": []
        }});
        assert_eq!(Ecosystem::PyPi.first_published(&pypi), Some("2012-01-01T00:00:00Z"));
        assert_eq!(Ecosystem::Npm.lookup_url("@types/node"), "https://registry.npmjs.org/@types%2Fnode");
    }
}
