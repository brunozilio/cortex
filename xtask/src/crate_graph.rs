use std::collections::BTreeSet;
use std::path::Path;

const ALLOWED: [(&str, &[&str]); 15] = [
    ("cli", &["platform", "content", "brand", "app", "console", "domain", "relay", "page"]),
    ("app", &["platform", "brand", "domain", "llm-providers", "agent-engine", "code-map"]),
    ("agent-engine", &["script", "content", "brand", "domain", "platform", "language-servers", "llm-providers"]),
    ("llm-providers", &["brand", "domain", "platform"]),
    ("domain", &["brand"]),
    ("language-servers", &["brand", "domain", "platform"]),
    ("content", &["brand", "script"]),
    ("console", &["brand", "platform", "page", "tunnel"]),
    ("relay", &["page", "tunnel", "brand"]),
    ("page", &[]),
    ("tunnel", &[]),
    ("code-map", &[]),
    ("platform", &[]),
    ("script", &[]),
    ("brand", &[]),
];

type Edges = BTreeSet<(String, String)>;

pub fn findings(root: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut manifests = Vec::new();
    for entry in std::fs::read_dir(root.join("crates"))? {
        let path = entry?.path();
        let manifest = path.join("Cargo.toml");
        if manifest.is_file()
            && let Some(name) = path.file_name()
        {
            manifests.push((name.to_string_lossy().into_owned(), std::fs::read_to_string(manifest)?));
        }
    }
    Ok(check(&edges(&manifests), &allowed()))
}

fn allowed() -> Edges {
    ALLOWED.iter().flat_map(|(from, targets)| targets.iter().map(|to| ((*from).to_owned(), (*to).to_owned()))).collect()
}

fn check(actual: &Edges, allowed: &Edges) -> Vec<String> {
    let forbidden = actual.difference(allowed).map(|(from, to)| format!("crate-edge {from} -> {to}"));
    let unused = allowed.difference(actual).map(|(from, to)| format!("crate-edge-unused {from} -> {to}"));
    forbidden.chain(unused).collect()
}

fn edges(manifests: &[(String, String)]) -> Edges {
    let workspace: BTreeSet<&str> = manifests.iter().map(|(name, _)| name.as_str()).collect();
    manifests
        .iter()
        .flat_map(|(from, manifest)| dependencies(manifest).into_iter().map(move |to| (from.clone(), to)))
        .filter(|(_, to)| workspace.contains(to.as_str()))
        .collect()
}

fn dependencies(manifest: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut listing = false;
    for line in manifest.lines().map(str::trim) {
        if let Some(section) = line.strip_prefix('[').and_then(|rest| rest.strip_suffix(']')) {
            listing = section == "dependencies" || section.ends_with(".dependencies");
            if let Some(name) = table_dependency(section) {
                names.insert(name);
            }
        } else if listing && let Some(name) = key(line) {
            names.insert(name);
        }
    }
    names
}

fn table_dependency(section: &str) -> Option<String> {
    let at = section.find("dependencies.")?;
    let before = &section[..at];
    (before.is_empty() || before.ends_with('.')).then(|| unquoted(&section[at + "dependencies.".len()..]))
}

fn key(line: &str) -> Option<String> {
    let end = line.find(['=', '.'])?;
    let name = unquoted(&line[..end]);
    (!name.is_empty()).then_some(name)
}

fn unquoted(name: &str) -> String {
    name.trim().trim_matches(['"', '\'']).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(pairs: &[(&str, &str)]) -> Edges {
        pairs.iter().map(|(from, to)| ((*from).to_owned(), (*to).to_owned())).collect()
    }

    fn manifests(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(name, manifest)| ((*name).to_owned(), (*manifest).to_owned())).collect()
    }

    #[test]
    fn a_graph_matching_the_table_passes() {
        let both = graph(&[("cli", "sdk"), ("sdk", "domain")]);
        assert!(check(&both, &both).is_empty());
    }

    #[test]
    fn names_an_edge_the_table_does_not_allow() {
        let actual = graph(&[("cli", "sdk"), ("domain", "cli")]);
        assert_eq!(check(&actual, &graph(&[("cli", "sdk")])), ["crate-edge domain -> cli"]);
    }

    #[test]
    fn names_an_allowed_edge_that_no_manifest_uses_any_more() {
        let allowed = graph(&[("cli", "sdk"), ("cli", "console")]);
        assert_eq!(check(&graph(&[("cli", "sdk")]), &allowed), ["crate-edge-unused cli -> console"]);
    }

    #[test]
    fn only_workspace_crates_are_edges() {
        let found = edges(&manifests(&[
            ("cli", "[dependencies]\nsdk.workspace = true\nserde = \"1\"\n"),
            ("sdk", "[dependencies]\nserde_json.workspace = true\n"),
        ]));
        assert_eq!(found, graph(&[("cli", "sdk")]));
    }

    #[test]
    fn dev_and_build_dependencies_are_not_edges() {
        let manifest = "[dependencies]\nsdk.workspace = true\n\n[dev-dependencies]\nbackend.workspace = true\n\n\
                        [build-dependencies]\nbrand.workspace = true\n\n[target.'cfg(unix)'.dev-dependencies]\n\
                        domain.workspace = true\n";
        assert_eq!(dependencies(manifest), BTreeSet::from(["sdk".to_owned()]));
    }

    #[test]
    fn every_way_of_writing_a_dependency_is_read() {
        let manifest = "[package]\nname = \"cli\"\n\n[dependencies]\nsdk.workspace = true\n\
                        domain = { path = \"../domain\" }\n\"brand\" = \"1\"\n\n[dependencies.console]\n\
                        workspace = true\n\n[target.'cfg(unix)'.dependencies]\nplatform = { workspace = true }\n\n\
                        [target.'cfg(windows)'.dependencies.script]\nworkspace = true\n\n[dev-dependencies.backend]\n\
                        workspace = true\n";
        let names: Vec<String> = dependencies(manifest).into_iter().collect();
        assert_eq!(names, ["brand", "console", "domain", "platform", "script", "sdk"]);
    }

    #[test]
    fn the_table_names_no_edge_twice() {
        let listed: usize = ALLOWED.iter().map(|(_, targets)| targets.len()).sum();
        assert_eq!(allowed().len(), listed);
    }
}
