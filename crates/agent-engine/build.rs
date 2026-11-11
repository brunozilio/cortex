use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let folder = Path::new(&manifest).join("src/tools/builtin");
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
    let listed: Vec<String> = names.iter().map(|name| format!("{name}::tools()")).collect();
    registry.push_str(&format!(
        "pub fn tools() -> Vec<Box<dyn crate::tools::Tool>> {{\n    [{}].into_iter().flatten().collect()\n}}\n",
        listed.join(", ")
    ));
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    if let Err(error) = std::fs::write(Path::new(&out).join("builtin_tools.rs"), registry) {
        panic!("cannot write the tool registry: {error}");
    }
}
