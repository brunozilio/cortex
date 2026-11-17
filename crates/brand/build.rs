use std::path::PathBuf;

type Pairs = Vec<(String, String)>;

fn values(text: &str) -> (Pairs, Vec<(String, Pairs)>) {
    let mut top = Vec::new();
    let mut sections: Vec<(String, Pairs)> = Vec::new();
    for line in text.lines().map(|line| line.split_once('#').map_or(line, |(before, _)| before).trim()) {
        if let Some(name) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            sections.push((name.trim().to_owned(), Vec::new()));
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let pair = (key.trim().to_owned(), value.trim().trim_matches('"').to_owned());
        match sections.last_mut() {
            Some((_, pairs)) => pairs.push(pair),
            None => top.push(pair),
        }
    }
    (top, sections)
}

fn identifier(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_lowercase())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn section_code(file: &std::path::Path, sections: &[(String, Pairs)]) -> String {
    let mut code = String::new();
    for (section, pairs) in sections {
        assert!(
            identifier(section),
            "a section of {} is lowercase letters, digits and `_`: [{section}]",
            file.display()
        );
        code.push_str(&format!("pub mod {section} {{\n"));
        for (key, value) in pairs {
            assert!(
                identifier(key),
                "a key of [{section}] in {} is lowercase letters, digits and `_`: {key}",
                file.display()
            );
            code.push_str(&format!("    pub const {}: &str = {value:?};\n", key.to_ascii_uppercase()));
        }
        code.push_str("}\n");
    }
    code
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let file = std::env::var("BRAND_FILE").map_or_else(|_| manifest.join("../../config.toml"), PathBuf::from);
    println!("cargo:rerun-if-changed={}", file.display());
    println!("cargo:rerun-if-env-changed=BRAND_FILE");
    let text = std::fs::read_to_string(&file).unwrap_or_else(|error| panic!("{}: {error}", file.display()));
    let (values, sections) = values(&text);
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
        ("NAME", get("name")),
        ("COMMAND", command.clone()),
        ("ENV", env.clone()),
        ("COMPANY", get("company")),
        ("AUTHOR", get("author")),
        ("DIR", format!(".{command}")),
        ("INSTRUCTIONS", format!("{env}.md")),
        ("LOCAL_INSTRUCTIONS", format!("{env}.local.md")),
        ("PLUGIN_DIR", format!(".{command}-plugin")),
    ];
    let mut code = String::new();
    for (name, value) in constants {
        code.push_str(&format!("pub const {name}: &str = {value:?};\n"));
        let lower = if name == "ENV" { "env_prefix".to_owned() } else { name.to_ascii_lowercase() };
        code.push_str(&format!("#[macro_export]\nmacro_rules! {lower} {{ () => {{ {value:?} }}; }}\n"));
    }
    code.push_str(&section_code(&file, &sections));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("brand.rs");
    std::fs::write(&out, code).unwrap_or_else(|error| panic!("{}: {error}", out.display()));
}
