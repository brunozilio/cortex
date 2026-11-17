use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const README: &str = "README.md";
const WEB_DOC: &str = "web/docs/en/environment-variables.md";

pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let prefix = crate::brand::Brand::read(root)?.env();
    let mut files = Vec::new();
    rust_files(&root.join("crates"), &mut files);
    let mut names = BTreeSet::new();
    for file in files {
        names.extend(read_by_code(&std::fs::read_to_string(file)?));
    }
    let readme = std::fs::read_to_string(root.join(README))?;
    let web = std::fs::read_to_string(root.join(WEB_DOC))?;
    Ok(undocumented(&names, &readme, &web, &prefix))
}

pub fn rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !["tests", "target", "node_modules"].contains(&name.as_str()) {
                rust_files(&path, files);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            files.push(path);
        }
    }
}

fn read_by_code(source: &str) -> BTreeSet<String> {
    let code = source.split("#[cfg(test)]").next().unwrap_or_default();
    let mut names = BTreeSet::new();
    for (at, call) in code.match_indices("brand::env(") {
        let rest = &code[at + call.len()..];
        let argument = rest.split(')').next().unwrap_or_default().trim();
        if let Some(literal) = argument.strip_prefix('"').and_then(|a| a.strip_suffix('"')) {
            names.insert(literal.to_owned());
        } else if let Some(value) = constant(code, argument) {
            names.insert(value);
        }
    }
    for (at, macro_call) in code.match_indices("env_prefix!(),") {
        let rest = code[at + macro_call.len()..].trim_start();
        if let Some(name) = rest.strip_prefix("\"_") {
            let name: String =
                name.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').collect();
            if !name.is_empty() {
                names.insert(name);
            }
        }
    }
    names
}

fn constant(code: &str, name: &str) -> Option<String> {
    let declared = format!("const {name}: &str = \"");
    let (_, rest) = code.split_once(&declared)?;
    rest.split('"').next().map(str::to_owned)
}

fn undocumented(names: &BTreeSet<String>, readme: &str, web: &str, prefix: &str) -> Vec<String> {
    let mut found = Vec::new();
    for name in names {
        if !readme.contains(&format!("{prefix}_{name}")) {
            found.push(format!("docs-drift env-var-not-documented {README} {prefix}_{name}"));
        }
        if !web.contains(&format!("{{{{ENV}}}}_{name}")) {
            found.push(format!("docs-drift env-var-not-documented {WEB_DOC} {{{{ENV}}}}_{name}"));
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_variables_code_reads_through_the_brand_prefix_are_found_and_tests_are_skipped() {
        let source = concat!(
            "const ENDPOINT: &str = \"OTLP_ENDPOINT\";\n",
            "fn a() { lookup(&brand::env(ENDPOINT)); std::env::var(brand::env(\"LOG\")); }\n",
            "const SECRETS: &str = concat!(brand::env_prefix!(), \"_SECRETS\");\n",
            "const ROOT: &str = concat!(\"${\", brand::env_prefix!(),\n    \"_PLUGIN_ROOT}\");\n",
            "#[cfg(test)]\nmod tests { fn t() { concat!(brand::env_prefix!(), \"_TEST_ONLY\"); } }\n",
        );
        assert_eq!(
            read_by_code(source).into_iter().collect::<Vec<_>>(),
            ["LOG", "OTLP_ENDPOINT", "PLUGIN_ROOT", "SECRETS"]
        );
    }

    #[test]
    fn a_variable_must_be_in_the_readme_and_the_user_docs() {
        let names: BTreeSet<String> = ["LOG".to_owned(), "THEME".to_owned()].into();
        let readme = "- `NOVA_LOG`: log filter.\n- `NOVA_THEME`: light.";
        let web = "| `{{ENV}}_LOG` | filter |";
        assert_eq!(
            undocumented(&names, readme, web, "NOVA"),
            ["docs-drift env-var-not-documented web/docs/en/environment-variables.md {{ENV}}_THEME"]
        );
    }
}
