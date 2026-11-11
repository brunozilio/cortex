use std::path::PathBuf;

fn features() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
    let folder = manifest.join("src/tui/features");
    println!("cargo:rerun-if-changed={}", folder.display());
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().join("mod.rs").is_file())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut registry = String::new();
    for name in &names {
        registry.push_str(&format!("#[path = {:?}]\nmod {name};\n", folder.join(name).join("mod.rs")));
    }
    let listed: Vec<String> = names.iter().map(|name| format!("{name}::feature()")).collect();
    registry.push_str(&format!("pub fn all() -> Vec<Feature> {{\n    vec![{}]\n}}\n", listed.join(", ")));
    let commands: String = names
        .iter()
        .filter(|name| folder.join(name).join("cli.rs").is_file())
        .map(|name| format!("        {name:?} => Some({name}::cli::run),\n"))
        .collect();
    registry.push_str(&format!(
        "pub type Command = fn(&sdk::Sdk, &std::path::Path, Vec<String>) -> Result<String, String>;\n\
         pub fn cli(name: &str) -> Option<Command> {{\n    \
         match name {{\n{commands}        _ => None,\n    }}\n}}\n"
    ));
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default()).join("features.rs");
    if let Err(error) = std::fs::write(&out, registry) {
        println!("cargo:warning=could not write the feature registry: {error}");
    }
}

fn main() {
    features();
}
