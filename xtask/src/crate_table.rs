use std::collections::BTreeSet;
use std::path::Path;

const DOCS: [&str; 1] = ["README.md"];

pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut crates = BTreeSet::new();
    for entry in std::fs::read_dir(root.join("crates"))? {
        let path = entry?.path();
        if path.join("Cargo.toml").is_file()
            && let Some(name) = path.file_name()
        {
            crates.insert(name.to_string_lossy().into_owned());
        }
    }
    let mut out = Vec::new();
    for doc in DOCS {
        out.extend(check(doc, &std::fs::read_to_string(root.join(doc))?, &crates));
    }
    Ok(out)
}

fn check(doc: &str, text: &str, crates: &BTreeSet<String>) -> Vec<String> {
    let listed = listed(text);
    let missing = crates.difference(&listed).map(|name| format!("crate-table-missing {doc} {name}"));
    let unknown = listed.difference(crates).map(|name| format!("crate-table-unknown {doc} {name}"));
    missing.chain(unknown).collect()
}

fn listed(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|row| row.split(" |").next())
        .flat_map(|cell| format!("`{cell}").split(", ").map(str::to_owned).collect::<Vec<_>>())
        .filter_map(|name| name.strip_prefix('`')?.strip_suffix('`').map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crates(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn a_table_naming_every_crate_passes() {
        let doc = "| Crate | What |\n|---|---|\n| `cli` | The binary. |\n| `sdk`, `domain` | Small. |\n";
        assert!(check("README.md", doc, &crates(&["cli", "sdk", "domain"])).is_empty());
    }

    #[test]
    fn names_crates_missing_from_the_table_and_rows_for_crates_that_are_gone() {
        let doc = "| `cli` | The binary. |\n| `subprocess`, `private-file` | Gone. |\n";
        assert_eq!(
            check("README.md", doc, &crates(&["cli", "platform"])),
            [
                "crate-table-missing README.md platform",
                "crate-table-unknown README.md private-file",
                "crate-table-unknown README.md subprocess",
            ]
        );
    }

    #[test]
    fn code_spans_in_the_description_are_not_crates() {
        let doc = "| `cli` | Runs `login` and `models`. |\n";
        assert!(check("README.md", doc, &crates(&["cli"])).is_empty());
    }
}
