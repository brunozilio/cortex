use std::path::Path;

const ARCHITECTURE: &str = "docs/architecture.md";
const VISION: &str = "docs/vision.md";
const FLAGS: &str = "docs/feature-flags.md";
const PROSE: [&str; 6] = ["README.md", "CLAUDE.md", ARCHITECTURE, VISION, "docs/GLOSSARY.md", "docs/threat-model.md"];
const NUMBERS: [&str; 20] = [
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
];
const FLAG_COUNTS: [&str; 6] =
    ["flags", "feature flags", "more flags", "constants", "whose name", "of them named experimental"];

pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let read = |path: &str| std::fs::read_to_string(root.join(path)).map_err(|e| format!("{path}: {e}"));
    let mut found = decisions(&records(root)?, &read(ARCHITECTURE)?, &read("CLAUDE.md")?);
    let command = crate::brand::Brand::read(root)?.command;
    found.extend(subcommands(&read("crates/cli/src/main.rs")?, &read("README.md")?, &read(VISION)?, &command));
    let crates = std::fs::read_dir(root.join("crates"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .count();
    for doc in PROSE.into_iter().chain([FLAGS]) {
        let text = read(doc)?;
        found.extend(crate_counts(doc, &text, crates));
        found.extend(flag_counts(doc, &text));
    }
    found.extend(flag_sections(&read(FLAGS)?));
    Ok(found)
}

fn records(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut names: Vec<String> = std::fs::read_dir(root.join("docs/decisions"))?
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| record_number(name).is_some())
        .collect();
    names.sort();
    Ok(names)
}

fn record_number(name: &str) -> Option<&str> {
    let number = name.get(..4)?;
    (number.bytes().all(|b| b.is_ascii_digit()) && name[4..].starts_with('-') && name.ends_with(".md"))
        .then_some(number)
}

fn decisions(records: &[String], architecture: &str, claude: &str) -> Vec<String> {
    let mut found: Vec<String> = records
        .iter()
        .filter(|name| !architecture.contains(&format!("decisions/{name}")))
        .map(|name| format!("docs-drift decision-not-in-target {ARCHITECTURE} {name}"))
        .collect();
    let highest = records.iter().filter_map(|name| record_number(name)).max();
    let stated = claude.split_once("`0001`…`").and_then(|(_, rest)| rest.get(..4));
    if let Some(highest) = highest
        && stated != Some(highest)
    {
        found.push(format!("docs-drift decision-range CLAUDE.md {} != {highest}", stated.unwrap_or("missing")));
    }
    found
}

fn subcommands(main: &str, readme: &str, vision: &str, command: &str) -> Vec<String> {
    let usage = readme.split_once("## Using it").and_then(|(_, rest)| rest.split("```").nth(1)).unwrap_or_default();
    let listed = vision.lines().find(|line| line.starts_with("- Subcommands:")).unwrap_or_default();
    let mut found = Vec::new();
    for name in actions(main) {
        let invoked = format!("{command} {name}");
        if !usage.lines().any(|line| line == invoked || line.starts_with(&format!("{invoked} "))) {
            found.push(format!("docs-drift subcommand-not-in-usage README.md {name}"));
        }
        if !listed.contains(&format!("`{name}`")) {
            found.push(format!("docs-drift subcommand-not-listed {VISION} {name}"));
        }
    }
    found
}

fn actions(main: &str) -> Vec<String> {
    let Some((_, body)) = main.split_once("enum Action {") else { return Vec::new() };
    body.lines()
        .take_while(|line| *line != "}")
        .filter_map(|line| line.strip_prefix("    "))
        .filter(|line| line.starts_with(|c: char| c.is_ascii_uppercase()))
        .map(|line| line.split(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or_default())
        .map(kebab)
        .collect()
}

fn kebab(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && index > 0 {
            out.push('-');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn counted<'t>(text: &'t str, nouns: &[&str]) -> Vec<(String, &'t str)> {
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    for noun in nouns {
        for (at, _) in lower.match_indices(&format!(" {noun}")) {
            let before = &lower[..at];
            let word = before.rsplit(|c: char| !c.is_ascii_alphanumeric()).next().unwrap_or_default();
            let after = lower[at + 1 + noun.len()..].chars().next();
            if after.is_some_and(|c| c.is_ascii_alphanumeric()) || word.is_empty() {
                continue;
            }
            if word.bytes().all(|b| b.is_ascii_digit()) || NUMBERS.contains(&word) {
                let start = at - word.len();
                found.push((word.to_owned(), &text[start..at + 1 + noun.len()]));
            }
        }
    }
    found
}

fn number(word: &str) -> Option<usize> {
    word.parse().ok().or_else(|| NUMBERS.iter().position(|name| *name == word).map(|index| index + 1))
}

fn crate_counts(doc: &str, text: &str, crates: usize) -> Vec<String> {
    counted(text, &["crates", "workspace crates"])
        .into_iter()
        .filter(|(word, _)| number(word) != Some(crates))
        .map(|(_, said)| format!("docs-drift crate-count {doc} \"{said}\" (there are {crates}; better, state none)"))
        .collect()
}

fn flag_counts(doc: &str, text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.starts_with('|'))
        .flat_map(|line| counted(line, &FLAG_COUNTS))
        .map(|(_, said)| format!("docs-drift flag-count {doc} \"{said}\" (point at {FLAGS} instead)"))
        .collect()
}

fn flag_sections(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut sections: Vec<(String, usize, usize)> = Vec::new();
    for line in text.lines() {
        if let Some(heading) = line.strip_prefix("#") {
            let heading = heading.trim_start_matches('#').trim();
            if let Some((title, rest)) = heading.rsplit_once(" (")
                && let Some(stated) = rest.strip_suffix(')').and_then(|n| n.parse().ok())
            {
                sections.push((title.to_owned(), stated, 0));
            } else {
                sections.push((String::new(), 0, 0));
            }
        } else if line.starts_with("| `")
            && let Some(section) = sections.last_mut()
        {
            section.2 += 1;
        }
    }
    for (title, stated, rows) in sections.iter().filter(|(title, _, _)| !title.is_empty()) {
        if stated != rows {
            found.push(format!("docs-drift flag-section {FLAGS} \"{title}\" says {stated}, lists {rows}"));
        }
        let summary = text.lines().find_map(|line| line.strip_prefix(&format!("| {title} | ")));
        let summarised = summary.and_then(|rest| rest.trim_end_matches(" |").trim().parse::<usize>().ok());
        if summarised.is_some_and(|summarised| summarised != *rows) {
            found.push(format!("docs-drift flag-summary {FLAGS} \"{title}\" lists {rows}"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn every_decision_record_has_a_target_row_and_the_working_agreement_names_the_highest() {
        let records = names(&["0001-a.md", "0002-b.md"]);
        let architecture = "| x | `decisions/0001-a.md` |\n| y | `decisions/0002-b.md` |";
        assert!(decisions(&records, architecture, "(`0001`…`0002`)").is_empty());
        assert_eq!(
            decisions(&records, "| x | `decisions/0001-a.md` |", "(`0001`…`0001`)"),
            [
                "docs-drift decision-not-in-target docs/architecture.md 0002-b.md",
                "docs-drift decision-range CLAUDE.md 0001 != 0002",
            ]
        );
        assert_eq!(
            decisions(&records, architecture, "no range"),
            ["docs-drift decision-range CLAUDE.md missing != 0002"]
        );
    }

    #[test]
    fn only_numbered_markdown_files_are_records() {
        assert_eq!(record_number("0012-remote.md"), Some("0012"));
        assert_eq!(record_number("README.md"), None);
        assert_eq!(record_number("0012.md"), None);
    }

    const MAIN: &str = "enum Action {\n    #[command(about = \"x\")]\n    Login,\n    Mcp {\n        #[command(subcommand)]\n        action: Option<X>,\n    },\n    SignOut,\n}\n";

    #[test]
    fn subcommands_are_read_from_the_action_enum_in_kebab_case() {
        assert_eq!(actions(MAIN), ["login", "mcp", "sign-out"]);
    }

    #[test]
    fn every_subcommand_is_in_the_readme_usage_and_the_vision_list() {
        let readme = "## Using it\n\n```sh\nnova login     # in\nnova mcp\nnova sign-out\n```\n";
        let vision = "- Subcommands: `login`, `mcp`, `sign-out`.\n";
        assert!(subcommands(MAIN, readme, vision, "nova").is_empty());
        assert_eq!(
            subcommands(
                MAIN,
                "## Using it\n\n```sh\nnova login\nnova mcpx\n```\nnova sign-out\n",
                "- Subcommands: `login`.",
                "nova"
            ),
            [
                "docs-drift subcommand-not-in-usage README.md mcp",
                "docs-drift subcommand-not-listed docs/vision.md mcp",
                "docs-drift subcommand-not-in-usage README.md sign-out",
                "docs-drift subcommand-not-listed docs/vision.md sign-out",
            ]
        );
    }

    #[test]
    fn a_crate_count_must_be_the_number_of_crates() {
        assert!(crate_counts("d.md", "Fifteen crates. Dependencies are one-way.", 15).is_empty());
        assert_eq!(
            crate_counts("d.md", "Twelve crates, and 3 workspace crates.", 15),
            [
                "docs-drift crate-count d.md \"Twelve crates\" (there are 15; better, state none)",
                "docs-drift crate-count d.md \"3 workspace crates\" (there are 15; better, state none)",
            ]
        );
        assert!(crate_counts("d.md", "Every crate under crates/ and the crates' tables.", 15).is_empty());
    }

    #[test]
    fn a_count_of_flags_is_not_stated_outside_the_inventory_tables() {
        assert_eq!(
            flag_counts("v.md", "The audit found about 34 feature flags, 17 of them named experimental."),
            [
                "docs-drift flag-count v.md \"34 feature flags\" (point at docs/feature-flags.md instead)",
                "docs-drift flag-count v.md \"17 of them named experimental\" (point at docs/feature-flags.md instead)",
            ]
        );
        assert!(flag_counts("v.md", "Dozens of feature flags; every flag whose name starts with x.").is_empty());
        assert!(flag_counts("v.md", "the flagship flags").is_empty());
        assert!(flag_counts("v.md", "| `web-x` | Six flags for one tool |").is_empty(), "a row of the inventory");
    }

    #[test]
    fn a_counted_section_of_the_inventory_lists_as_many_rows_as_it_and_the_summary_say() {
        let text = "| Make permanent | 2 |\n| Remove | 1 |\n### Make permanent (2)\n| Flag |\n| `a` |\n| `b` |\n### Remove (1)\n| `c` |\n### Where read\n| `x` |\n";
        assert!(flag_sections(text).is_empty());
        let drifted = text
            .replace("| Make permanent | 2 |", "| Make permanent | 3 |")
            .replace("### Remove (1)", "### Remove (2)");
        assert_eq!(
            flag_sections(&drifted),
            [
                "docs-drift flag-summary docs/feature-flags.md \"Make permanent\" lists 2",
                "docs-drift flag-section docs/feature-flags.md \"Remove\" says 2, lists 1",
            ]
        );
    }
}
