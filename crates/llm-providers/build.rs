use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let folder = Path::new(&manifest).join("src/providers");
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
        registry.push_str(&format!("#[path = {:?}]\npub mod {name};\n", folder.join(name).join("mod.rs")));
    }
    let listed: Vec<String> = names.iter().map(|name| format!("&{name}::Provider")).collect();
    registry.push_str(&format!("pub static ALL: &[&dyn crate::Provider] = &[{}];\n", listed.join(", ")));
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    if let Err(error) = std::fs::write(Path::new(&out).join("registry.rs"), registry) {
        panic!("cannot write the provider registry: {error}");
    }
}
