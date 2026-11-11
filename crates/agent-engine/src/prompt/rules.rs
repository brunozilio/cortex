use globset::{Glob, GlobSetBuilder};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Rule {
    pub path: PathBuf,
    #[serde(default)]
    pub base: Option<PathBuf>,
    pub text: String,
    pub patterns: Vec<String>,
    #[serde(default)]
    pub error: Option<String>,
}
pub fn discover(cwd: &Path, home: Option<&Path>) -> Vec<Rule> {
    let mut roots = Vec::new();
    if let Some(home) = home {
        roots.push((home.join(concat!(brand::dir!(), "/rules")), None));
    }
    let mut ancestors: Vec<_> = cwd.ancestors().collect();
    ancestors.reverse();
    for directory in ancestors {
        roots.push((directory.join(concat!(brand::dir!(), "/rules")), Some(directory.to_path_buf())));
    }
    let mut rules = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (root, base) in roots {
        let mut files: Vec<_> = ignore::WalkBuilder::new(root)
            .hidden(false)
            .max_depth(Some(8))
            .build()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_some_and(|t| t.is_file()) && e.path().extension().is_some_and(|e| e == "md"))
            .map(|e| e.into_path())
            .collect();
        files.sort();
        for path in files {
            if !seen.insert(path.clone()) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let (patterns, body, error) = match super::markdown::frontmatter(&text) {
                Ok((front, body)) => match super::markdown::list(&front["paths"]) {
                    Ok(list) => (list.unwrap_or_default(), body.to_owned(), None),
                    Err(error) => (Vec::new(), String::new(), Some(error)),
                },
                Err(error) => (Vec::new(), String::new(), Some(error)),
            };
            rules.push(Rule { path, base: base.clone(), text: body.trim().into(), patterns, error });
        }
    }
    rules.extend(super::instruction_files::nested(cwd));
    rules
}
impl Rule {
    pub fn applies(&self, cwd: &Path, path: Option<&Path>) -> Result<bool, String> {
        if let Some(error) = &self.error {
            return Err(format!("{}: {error}", self.path.display()));
        }
        if self.patterns.is_empty() {
            return Ok(true);
        }
        let Some(path) = path else {
            return Ok(false);
        };
        let mut globs = GlobSetBuilder::new();
        for pattern in &self.patterns {
            globs.add(Glob::new(pattern).map_err(|e| format!("{}: {e}", self.path.display()))?);
        }
        let absolute = if path.is_absolute() { path.to_path_buf() } else { cwd.join(path) };
        let path = normalize(&absolute);
        let base = normalize(self.base.as_deref().unwrap_or(cwd));
        let globs = globs.build().map_err(|e| e.to_string())?;
        let lexical = path.strip_prefix(&base).is_ok_and(|relative| globs.is_match(relative));
        let resolved = resolve_existing(&absolute);
        let resolved_base = resolve_existing(&base);
        Ok(lexical || resolved.strip_prefix(resolved_base).is_ok_and(|relative| globs.is_match(relative)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loads_unconditional_and_path_scoped_rules_recursively() {
        let dir = tempfile::tempdir().expect("valid test fixture");
        let rules = dir.path().join(concat!(brand::dir!(), "/rules/nested"));
        std::fs::create_dir_all(&rules).expect("valid test fixture");
        std::fs::write(
            rules.join("rust.md"),
            "---\npaths:\n  - \"src/**/*.rs\"\n  - \"tests/**/*.rs\"\n---\nUse checked arithmetic.",
        )
        .expect("valid test fixture");
        let found = discover(dir.path(), None);
        assert_eq!(found.len(), 1);
        assert!(!found[0].applies(dir.path(), None).expect("valid test fixture"));
        assert!(found[0].applies(dir.path(), Some(&dir.path().join("src/a.rs"))).expect("valid test fixture"));
        assert!(!found[0].applies(dir.path(), Some(&dir.path().join("README.md"))).expect("valid test fixture"));
    }
}
#[cfg(test)]
mod additional_tests {
    use super::*;
    #[test]
    fn yaml_brace_globs_and_nested_instructions_remain_scoped() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(concat!(brand::dir!(), "/rules"))).expect("rules");
        std::fs::create_dir_all(repo.path().join("src")).expect("src");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/rules/web.md")),
            "---\npaths:\n  - 'src/**/*.{ts,tsx}'\n  - 'lib/**/*.ts'\n---\nWeb rules",
        )
        .expect("rule");
        std::fs::write(repo.path().join(concat!("src/", brand::instructions!())), "Nested source instructions")
            .expect("nested");
        let rules = discover(repo.path(), None);
        assert_eq!(rules.len(), 2);
        let web = rules.iter().find(|r| r.text == "Web rules").expect("web");
        assert!(web.applies(repo.path(), Some(&repo.path().join("src/view.tsx"))).expect("glob"));
        assert!(!web.applies(repo.path(), Some(&repo.path().join("src/backend.rs"))).expect("glob"));
        let nested = rules.iter().find(|r| r.text == "Nested source instructions").expect("nested");
        assert!(nested.applies(repo.path(), Some(&repo.path().join("src/backend.rs"))).expect("nested applies"));
        assert!(!nested.applies(repo.path(), Some(&repo.path().join("README.md"))).expect("nested scope"));
    }
    #[test]
    fn invalid_rule_metadata_fails_closed() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(concat!(brand::dir!(), "/rules"))).expect("rules");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/rules/invalid.md")),
            "---\npaths: [unterminated\n---\nDo not broaden this rule.",
        )
        .expect("rule");
        let rule = discover(repo.path(), None).remove(0);
        assert!(rule.applies(repo.path(), Some(&repo.path().join("file"))).is_err());
    }
}

fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            _ => result.push(part.as_os_str()),
        }
    }
    result
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn ancestor_rules_use_the_declaring_project_and_normalize_tool_paths() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(concat!(brand::dir!(), "/rules"))).expect("rules");
        std::fs::create_dir_all(repo.path().join("src")).expect("src");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/rules/source.md")),
            "---\npaths: ['src/**']\n---\nSource rule",
        )
        .expect("rule");
        let nested = repo.path().join("src");
        let rules = discover(&nested, None);
        let rule = rules.iter().find(|rule| rule.text == "Source rule").expect("ancestor rule");
        assert!(rule.applies(&nested, Some(&nested.join("a.rs"))).expect("normal path"));
        assert!(rule.applies(repo.path(), Some(&repo.path().join("other/../src/a.rs"))).expect("dotdot path"));
        assert!(!rule.applies(&nested, Some(&nested.join("../README.md"))).expect("outside src"));
    }
}

fn resolve_existing(path: &Path) -> PathBuf {
    let mut ancestor = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        if let Ok(mut resolved) = ancestor.canonicalize() {
            for name in missing.iter().rev() {
                resolved.push(name);
            }
            return normalize(&resolved);
        }
        let Some(name) = ancestor.file_name().map(|name| name.to_os_string()) else {
            return normalize(path);
        };
        missing.push(name);
        if !ancestor.pop() {
            return normalize(path);
        }
    }
}
#[cfg(all(test, unix))]
mod symlink_tests {
    use super::*;
    #[test]
    fn symlink_parent_of_new_file_receives_target_rules() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::create_dir_all(repo.path().join(concat!(brand::dir!(), "/rules"))).expect("rules");
        std::fs::create_dir_all(repo.path().join("src")).expect("src");
        std::os::unix::fs::symlink(repo.path().join("src"), repo.path().join("alias")).expect("alias");
        std::fs::write(
            repo.path().join(concat!(brand::dir!(), "/rules/source.md")),
            "---\npaths: ['src/**']\n---\nSource rule",
        )
        .expect("rule");
        let rule = discover(repo.path(), None).remove(0);
        assert!(rule.applies(repo.path(), Some(&repo.path().join("alias/new.rs"))).expect("real path rule"));
    }
}
