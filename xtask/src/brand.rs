//! `xtask brand`: spells `brand.toml` out where a file cannot read it: the
//! binary's name, the workspace's author, release-please's package name,
//! install.sh's defaults, README.md (from docs/README.template.md) and the
//! repository's own project folder (`.{command}/`, renamed with its
//! references). `xtask lint` checks they are current, and that nothing
//! else names the product.

use std::path::{Path, PathBuf};

/// brand.toml's values and the names derived from them.
#[derive(Debug, Clone)]
pub struct Brand {
    pub name: String,
    pub command: String,
    pub company: String,
    pub author: String,
    pub repository: String,
}

impl Brand {
    pub fn read(root: &Path) -> Result<Self, String> {
        let file = root.join("brand.toml");
        let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let values: Vec<(String, String)> = text
            .lines()
            .map(|line| line.split_once('#').map_or(line, |(before, _)| before).trim())
            .filter_map(|line| line.split_once('='))
            .map(|(key, value)| (key.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
            .collect();
        let get = |key: &str| {
            values
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
                .filter(|value| !value.is_empty())
                .ok_or_else(|| format!("brand.toml has no `{key}`"))
        };
        Ok(Self {
            name: get("name")?,
            command: get("command")?,
            company: get("company")?,
            author: get("author")?,
            repository: get("repository")?,
        })
    }

    pub fn env(&self) -> String {
        self.command.to_ascii_uppercase().replace('-', "_")
    }

    /// The words that name the product or its owner.
    pub fn words(&self) -> Vec<String> {
        let mut words = vec![self.name.clone(), self.company.clone(), self.author.clone(), self.repository.clone()];
        if let Some((owner, _)) = self.repository.split_once('/') {
            words.push(owner.to_owned());
        }
        words.push(self.command.clone());
        words.push(self.env());
        words.retain(|word| word.len() >= 4);
        words
    }

    /// `template` with `{{NAME}}`, `{{COMMAND}}`… filled in.
    pub fn fill(&self, template: &str) -> String {
        let env = self.env();
        [
            ("{{NAME}}", self.name.clone()),
            ("{{COMMAND}}", self.command.clone()),
            ("{{DIR}}", format!(".{}", self.command)),
            ("{{INSTRUCTIONS}}", format!("{env}.md")),
            ("{{LOCAL_INSTRUCTIONS}}", format!("{env}.local.md")),
            ("{{PLUGIN_DIR}}", format!(".{}-plugin", self.command)),
            ("{{ENV}}", env.clone()),
            ("{{COMPANY}}", self.company.clone()),
            ("{{AUTHOR}}", self.author.clone()),
            ("{{REPOSITORY}}", self.repository.clone()),
        ]
        .iter()
        .fold(template.to_owned(), |text, (placeholder, value)| text.replace(placeholder, value))
    }
}

/// A file's content as brand.toml wants it, when it differs.
struct Change {
    path: PathBuf,
    content: String,
}

/// The command the binary is named now: the brand last applied.
fn applied_command(root: &Path) -> Result<String, String> {
    let manifest = std::fs::read_to_string(root.join("crates/cli/Cargo.toml")).map_err(|e| e.to_string())?;
    let bin = manifest.split("[[bin]]").nth(1).ok_or("crates/cli/Cargo.toml has no [[bin]]")?;
    bin.lines()
        .find_map(|line| line.trim().strip_prefix("name = \"").and_then(|rest| rest.strip_suffix('"')))
        .map(str::to_owned)
        .ok_or_else(|| "crates/cli/Cargo.toml's [[bin]] has no name".into())
}

/// Replaces the value of the first line starting with `key` in `text`.
fn set_line(text: &str, key: &str, line: &str) -> String {
    text.lines()
        .scan(false, |done, current| {
            Some(if !*done && current.trim_start().starts_with(key) {
                *done = true;
                line.to_owned()
            } else {
                current.to_owned()
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if text.ends_with('\n') { "\n" } else { "" }
}

fn changes(root: &Path, brand: &Brand) -> Result<Vec<Change>, String> {
    let read = |path: &str| std::fs::read_to_string(root.join(path)).map_err(|e| format!("{path}: {e}"));
    let mut wanted = Vec::new();
    let manifest = read("crates/cli/Cargo.toml")?;
    let old = applied_command(root)?;
    let (head, bin) = manifest.split_once("[[bin]]").ok_or("crates/cli/Cargo.toml has no [[bin]]")?;
    let bin = bin.replacen(&format!("name = \"{old}\""), &format!("name = \"{}\"", brand.command), 1);
    wanted.push(("crates/cli/Cargo.toml", format!("{head}[[bin]]{bin}")));
    wanted.push((
        "Cargo.toml",
        set_line(&read("Cargo.toml")?, "authors = ", &format!("authors = [\"{}\"]", brand.author)),
    ));
    let config = read("release-please-config.json")?;
    let config = set_line(&config, "\"package-name\"", &format!("      \"package-name\": \"{}\",", brand.command));
    wanted.push(("release-please-config.json", config));
    let mut install = read("install.sh")?;
    for (key, value) in [("NAME=", &brand.name), ("COMMAND=", &brand.command), ("REPO=", &brand.repository)] {
        install = set_line(&install, key, &format!("{key}\"{value}\""));
    }
    wanted.push(("install.sh", install));
    wanted.push(("README.md", brand.fill(&read("docs/README.template.md")?)));
    Ok(wanted
        .into_iter()
        .filter_map(|(path, content)| {
            let path = root.join(path);
            (std::fs::read_to_string(&path).ok().as_deref() != Some(content.as_str()))
                .then_some(Change { path, content })
        })
        .collect())
}

/// What `brand` would change: empty when every file is current.
pub fn stale(root: &Path) -> Result<Vec<String>, String> {
    let brand = Brand::read(root)?;
    Ok(changes(root, &brand)?
        .into_iter()
        .map(|change| change.path.strip_prefix(root).unwrap_or(&change.path).display().to_string())
        .collect())
}

/// Applies brand.toml.
pub fn run(root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let brand = Brand::read(root)?;
    let old = applied_command(root)?;
    for change in changes(root, &brand)? {
        std::fs::write(&change.path, change.content)?;
        println!("brand: wrote {}", change.path.strip_prefix(root).unwrap_or(&change.path).display());
    }
    if old != brand.command {
        rename_project_folder(root, &old, &brand.command)?;
    }
    println!("brand: {} (`{}`)", brand.name, brand.command);
    Ok(())
}

/// Moves the repository's own `.<old>/` (and its `<OLD>.md`) to the new
/// names, with every reference to them in it, CLAUDE.md and `.claude/`.
fn rename_project_folder(root: &Path, old: &str, new: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (old_dir, new_dir) = (root.join(format!(".{old}")), root.join(format!(".{new}")));
    if !old_dir.is_dir() || new_dir.exists() {
        return Ok(());
    }
    std::fs::rename(&old_dir, &new_dir)?;
    let (old_env, new_env) = (old.to_ascii_uppercase().replace('-', "_"), new.to_ascii_uppercase().replace('-', "_"));
    let instructions = new_dir.join(format!("{old_env}.md"));
    if instructions.is_file() {
        std::fs::rename(&instructions, new_dir.join(format!("{new_env}.md")))?;
    }
    let swap = |text: &str| {
        text.replace(&format!(".{old}/"), &format!(".{new}/"))
            .replace(&format!("{old_env}.md"), &format!("{new_env}.md"))
            .replace(&format!("{old_env}_"), &format!("{new_env}_"))
    };
    let mut files = vec![root.join("CLAUDE.md"), root.join("AGENTS.md")];
    collect(&new_dir, &mut files);
    for file in files {
        if let Ok(text) = std::fs::read_to_string(&file) {
            let swapped = swap(&text);
            if swapped != text {
                std::fs::write(&file, swapped)?;
            }
        }
    }
    #[cfg(unix)]
    for link in ["agents", "hooks", "skills"] {
        let path = root.join(".claude").join(link);
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            std::fs::remove_file(&path)?;
            std::os::unix::fs::symlink(format!("../.{new}/{link}"), &path)?;
        }
    }
    println!("brand: moved .{old}/ to .{new}/");
    Ok(())
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brand() -> Brand {
        Brand {
            name: "Nova".into(),
            command: "nova".into(),
            company: "Acme".into(),
            author: "Ana Souza".into(),
            repository: "acme/nova".into(),
        }
    }

    #[test]
    fn templates_are_filled_from_the_brand() {
        let filled = brand().fill("{{NAME}}: `{{COMMAND}}`, {{DIR}}/{{INSTRUCTIONS}}, {{ENV}}_X, {{REPOSITORY}}");
        assert_eq!(filled, "Nova: `nova`, .nova/NOVA.md, NOVA_X, acme/nova");
    }

    #[test]
    fn a_line_is_replaced_once() {
        assert_eq!(set_line("a\nNAME=\"x\"\nNAME=\"y\"\n", "NAME=", "NAME=\"z\""), "a\nNAME=\"z\"\nNAME=\"y\"\n");
    }

    #[test]
    fn the_words_include_the_owner_but_not_short_ones() {
        let words = brand().words();
        assert!(words.contains(&"acme".to_owned()) && words.contains(&"NOVA".to_owned()));
        assert!(words.iter().all(|word| word.len() >= 4));
    }
}

/// Files that spell the brand out on purpose: brand.toml, what `brand`
/// writes, history, and the repository's own project folder.
fn allowed(path: &str, brand: &Brand) -> bool {
    let own = format!(".{}/", brand.command);
    ["brand.toml", "CHANGELOG.md", "Cargo.lock", "Cargo.toml", "crates/cli/Cargo.toml", "release-please-config.json"]
        .contains(&path)
        || ["README.md", "CLAUDE.md", "AGENTS.md"].contains(&path)
        || path.starts_with(&own)
        || path.starts_with(".claude/")
}

/// Whether `word` is in `text` on its own (not inside a longer word).
fn names(text: &str, word: &str) -> bool {
    let (text, word) = (text.to_ascii_lowercase(), word.to_ascii_lowercase());
    text.match_indices(&word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

/// `brand-stale` and `brand-name` findings.
pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let brand = Brand::read(root)?;
    let mut found: Vec<String> = stale(root)?.into_iter().map(|path| format!("brand-stale {path}")).collect();
    let listed = std::process::Command::new("git").arg("ls-files").current_dir(root).output()?;
    let words = brand.words();
    for path in String::from_utf8_lossy(&listed.stdout).lines() {
        if allowed(path, &brand) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(path)) else { continue };
        // install.sh's defaults are written by `brand`.
        let text: String = if path == "install.sh" {
            text.lines()
                .filter(|line| !["NAME=", "COMMAND=", "REPO="].iter().any(|key| line.starts_with(key)))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            text
        };
        if words.iter().any(|word| names(&text, word)) {
            found.push(format!("brand-name {path}"));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod name_tests {
    use super::names;

    #[test]
    fn a_name_counts_on_its_own_only() {
        assert!(names("Run acme login", "acme"));
        assert!(names("ACME_X=1", "acme"));
        assert!(names("brand::NAME and ~/.acme/", "acme"));
        assert!(!names("acmes", "acme"));
        assert!(!names("supernova", "nova"));
    }
}
