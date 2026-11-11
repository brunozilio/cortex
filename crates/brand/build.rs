//! Reads the workspace's `brand.toml` (or the file `BRAND_FILE` names) and
//! writes its values, and the names derived from them, as constants.

use std::path::PathBuf;

/// The `key = "value"` lines of a flat TOML file.
fn values(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(|line| line.split_once('#').map_or(line, |(before, _)| before).trim())
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
        .collect()
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let file = std::env::var("BRAND_FILE").map_or_else(|_| manifest.join("../../brand.toml"), PathBuf::from);
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rerun-if-env-changed=BRAND_FILE");
    let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let values = values(&text);
    let get = |key: &str| {
        values
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| panic!("{} has no `{key}`", file.display()))
    };
    let command = get("command");
    let valid = command.starts_with(|c: char| c.is_ascii_lowercase())
        && command.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    assert!(valid, "`command` in {} must be lowercase letters, digits and dashes: {command}", file.display());
    let env = command.to_ascii_uppercase().replace('-', "_");
    let constants = [
        ("NAME", "How texts name the product.", get("name")),
        ("COMMAND", "The command, and the name of its folders.", command.clone()),
        ("ENV", "The prefix of its environment variables (`ENV_NAME`).", env.clone()),
        ("COMPANY", "Who makes it.", get("company")),
        ("AUTHOR", "Who wrote it.", get("author")),
        ("REPOSITORY", "The GitHub repository (owner/name) of its releases.", get("repository")),
        ("DIR", "The folder of its files in a project and in the home folder.", format!(".{command}")),
        ("INSTRUCTIONS", "The instructions file a project gives the agent.", format!("{env}.md")),
        ("LOCAL_INSTRUCTIONS", "The instructions file kept out of version control.", format!("{env}.local.md")),
        ("PLUGIN_DIR", "The folder holding a plugin's manifest.", format!(".{command}-plugin")),
    ];
    let mut code = String::new();
    for (name, doc, value) in constants {
        code.push_str(&format!("/// {doc}\npub const {name}: &str = {value:?};\n"));
        // The same as a literal, for `concat!` (constants, format strings).
        let lower = if name == "ENV" { "env_prefix".to_owned() } else { name.to_ascii_lowercase() };
        code.push_str(&format!(
            "/// [`{name}`] as a literal, for `concat!`.\n#[macro_export]\nmacro_rules! {lower} {{ () => {{ {value:?} }}; }}\n"
        ));
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("brand.rs");
    std::fs::write(&out, code).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
}
