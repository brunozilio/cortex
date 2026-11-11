use std::path::{Path, PathBuf};

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

fn modules(folder: &Path, names: &[String]) -> String {
    names.iter().map(|name| format!("#[path = {:?}]\npub mod {name};\n", folder.join(name).join("mod.rs"))).collect()
}

fn write(out: &Path, file: &str, text: String) {
    if let Err(error) = std::fs::write(out.join(file), text) {
        panic!("cannot write {file}: {error}");
    }
}

fn tools(src: &Path) -> String {
    let folder = src.join("tools/builtin");
    let names = folders(&folder);
    let listed: Vec<String> = names.iter().map(|name| format!("{name}::tools()")).collect();
    let reviewers: Vec<String> = names
        .iter()
        .filter(|name| folder.join(name).join("review.rs").is_file())
        .map(|name| format!("{name}::review::reviewers()"))
        .collect();
    format!(
        "{}pub fn tools() -> Vec<Box<dyn crate::tools::Tool>> {{\n    [{}].into_iter().flatten().collect()\n}}\n\
         pub fn reviewers() -> Vec<Box<dyn crate::agent::Review>> {{\n    \
         let all: [Vec<Box<dyn crate::agent::Review>>; {}] = [{}];\n    all.into_iter().flatten().collect()\n}}\n",
        modules(&folder, &names),
        listed.join(", "),
        reviewers.len(),
        reviewers.join(", ")
    )
}

fn providers(src: &Path) -> (String, Vec<String>) {
    let folder = src.join("providers");
    let names = folders(&folder);
    let listed: Vec<String> = names.iter().map(|name| format!("&{name}::Provider")).collect();
    let wires = names
        .iter()
        .filter(|name| folder.join(name).join("wire/mod.rs").is_file())
        .map(|name| format!("crate::providers::{name}::wire"))
        .collect();
    let registry = format!(
        "{}pub static ALL: &[&dyn llm_providers::Provider] = &[{}];\n",
        modules(&folder, &names),
        listed.join(", ")
    );
    (registry, wires)
}

fn wires(src: &Path, owned: Vec<String>) -> String {
    let folder = src.join("model/wires");
    let names = folders(&folder);
    let paths: Vec<String> = names.iter().map(|name| format!("self::{name}")).chain(owned).collect();
    let arms: String = paths
        .iter()
        .map(|path| {
            format!("    if name == {path}::NAME {{\n        return Some({path}::open(http, model));\n    }}\n")
        })
        .collect();
    format!(
        "{}pub fn open(\n    name: &str,\n    http: std::sync::Arc<dyn crate::model::http::Http>,\n    \
         model: Option<&domain::ModelOption>,\n) -> Option<std::sync::Arc<dyn crate::model::ModelApi>> {{\n    \
         let _ = (name, &http, model);\n{arms}    None\n}}\n",
        modules(&folder, &names)
    )
}

fn main() {
    let src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default()).join("src");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_default());
    write(&out, "builtin_tools.rs", tools(&src));
    let (registry, owned) = providers(&src);
    write(&out, "providers.rs", registry);
    write(&out, "wires.rs", wires(&src, owned));
}
