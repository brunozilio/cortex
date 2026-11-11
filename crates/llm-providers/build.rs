use std::path::Path;

fn folders(folder: &Path) -> Vec<String> {
    println!("cargo:rerun-if-changed={}", folder.display());
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.path().join("mod.rs").is_file())
                .filter_map(|entry| entry.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn registry(folder: &Path, kind: &str) -> String {
    let names = folders(folder);
    let mut registry = String::new();
    for name in &names {
        registry.push_str(&format!("#[path = {:?}]\npub mod {name};\n", folder.join(name).join("mod.rs")));
    }
    let listed: Vec<String> = names.iter().map(|name| format!("&{name}::{kind}")).collect();
    registry.push_str(&format!("pub static ALL: &[&dyn crate::{kind}] = &[{}];\n", listed.join(", ")));
    registry
}

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let out = std::env::var("OUT_DIR").unwrap_or_default();
    let written = registry(&Path::new(&manifest).join("src/vendors"), "Vendor");
    if let Err(error) = std::fs::write(Path::new(&out).join("vendors.rs"), written) {
        panic!("cannot write the vendor registry: {error}");
    }
}
