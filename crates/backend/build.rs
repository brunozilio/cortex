use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let folder = Path::new(&manifest).join("src/features");
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
    let arms: String = names
        .iter()
        .map(|name| {
            format!("    if feature == {name:?} {{\n        return Some({name}::handle(client, principal, input));\n    }}\n")
        })
        .collect();
    let forgets: String = names.iter().map(|name| format!("    {name}::forget(client);\n")).collect();
    registry.push_str(&format!(
        "pub fn handle(feature: &str, client: u64, principal: &domain::Principal, input: serde_json::Value) \
         -> Option<Result<serde_json::Value, String>> {{\n    let _ = (feature, client, principal, &input);\n{arms}    None\n}}\n\
         pub fn forget(client: u64) {{\n    let _ = client;\n{forgets}}}\n"
    ));
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    if let Err(error) = std::fs::write(Path::new(&out).join("features.rs"), registry) {
        panic!("cannot write the feature registry: {error}");
    }
}
