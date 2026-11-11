use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSpec {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub extension_to_language: BTreeMap<String, String>,
    #[serde(default)]
    pub root_markers: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub extension_to_language: Option<BTreeMap<String, String>>,
    pub root_markers: Option<Vec<String>>,
    pub env: Option<BTreeMap<String, String>>,
    pub disabled: bool,
}

impl ServerConfig {
    pub fn without_commands(self) -> Self {
        Self { command: None, args: None, env: None, ..self }
    }
}

fn built_in(name: &str, command: &str, args: &[&str], languages: &[(&str, &str)], markers: &[&str]) -> ServerSpec {
    let owned = |items: &[&str]| items.iter().map(|item| (*item).to_owned()).collect();
    ServerSpec {
        name: name.into(),
        command: command.into(),
        args: owned(args),
        extension_to_language: languages.iter().map(|(ext, id)| ((*ext).to_owned(), (*id).to_owned())).collect(),
        root_markers: owned(markers),
        env: BTreeMap::new(),
    }
}

pub fn defaults() -> Vec<ServerSpec> {
    vec![
        built_in("rust-analyzer", "rust-analyzer", &[], &[(".rs", "rust")], &["Cargo.toml"]),
        built_in(
            "typescript",
            "typescript-language-server",
            &["--stdio"],
            &[
                (".ts", "typescript"),
                (".tsx", "typescriptreact"),
                (".js", "javascript"),
                (".jsx", "javascriptreact"),
                (".mjs", "javascript"),
                (".cjs", "javascript"),
            ],
            &["tsconfig.json", "jsconfig.json", "package.json"],
        ),
        built_in(
            "pyright",
            "pyright-langserver",
            &["--stdio"],
            &[(".py", "python"), (".pyi", "python")],
            &["pyproject.toml", "setup.py", "setup.cfg", "requirements.txt"],
        ),
        built_in("gopls", "gopls", &[], &[(".go", "go")], &["go.mod", "go.work"]),
        built_in(
            "clangd",
            "clangd",
            &[],
            &[(".c", "c"), (".h", "c"), (".cc", "cpp"), (".cpp", "cpp"), (".hpp", "cpp")],
            &["compile_commands.json", "CMakeLists.txt"],
        ),
    ]
}

pub fn servers(defaults: Vec<ServerSpec>, layers: &[BTreeMap<String, ServerConfig>]) -> Vec<ServerSpec> {
    let mut specs = defaults;
    for (name, config) in layers.iter().flatten() {
        let at = specs.iter().position(|spec| &spec.name == name);
        if config.disabled {
            if let Some(at) = at {
                specs.remove(at);
            }
            continue;
        }
        match at {
            Some(at) => apply(&mut specs[at], config),
            None => specs.extend(new_server(name, config)),
        }
    }
    specs
}

fn apply(spec: &mut ServerSpec, config: &ServerConfig) {
    let config = config.clone();
    if let Some(command) = config.command {
        spec.command = command;
    }
    if let Some(args) = config.args {
        spec.args = args;
    }
    if let Some(languages) = config.extension_to_language {
        spec.extension_to_language = languages;
    }
    if let Some(markers) = config.root_markers {
        spec.root_markers = markers;
    }
    if let Some(env) = config.env {
        spec.env.extend(env);
    }
}

fn new_server(name: &str, config: &ServerConfig) -> Option<ServerSpec> {
    let mut spec = ServerSpec {
        name: name.to_owned(),
        command: config.command.clone()?,
        args: Vec::new(),
        extension_to_language: config.extension_to_language.clone().filter(|map| !map.is_empty())?,
        root_markers: Vec::new(),
        env: BTreeMap::new(),
    };
    apply(&mut spec, config);
    Some(spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(json: serde_json::Value) -> BTreeMap<String, ServerConfig> {
        serde_json::from_value(json).expect("layer")
    }

    fn named<'a>(specs: &'a [ServerSpec], name: &str) -> Option<&'a ServerSpec> {
        specs.iter().find(|spec| spec.name == name)
    }

    #[test]
    fn a_setting_changes_a_built_in_server_field_by_field() {
        let specs = servers(
            defaults(),
            &[layer(serde_json::json!({"rust-analyzer": {"command": "/opt/ra", "env": {"RA_LOG": "error"}}}))],
        );
        let rust = named(&specs, "rust-analyzer").expect("rust-analyzer");
        assert_eq!(rust.command, "/opt/ra");
        assert_eq!(rust.env.get("RA_LOG").map(String::as_str), Some("error"));
        assert_eq!(rust.extension_to_language.get(".rs").map(String::as_str), Some("rust"));
        assert_eq!(rust.root_markers, ["Cargo.toml"]);
    }

    #[test]
    fn a_later_layer_adds_removes_and_wins_over_an_earlier_one() {
        let user = layer(serde_json::json!({
            "gopls": {"disabled": true},
            "zls": {"command": "zls", "extensionToLanguage": {".zig": "zig"}},
            "half": {"command": "half"}
        }));
        let project = layer(serde_json::json!({"zls": {"args": ["--stdio"]}, "clangd": {"disabled": true}}));
        let specs = servers(defaults(), &[user, project]);
        assert!(named(&specs, "gopls").is_none());
        assert!(named(&specs, "clangd").is_none());
        assert!(named(&specs, "half").is_none(), "a new server needs a command and its languages");
        let zig = named(&specs, "zls").expect("zls");
        assert_eq!((zig.command.as_str(), zig.args.as_slice()), ("zls", ["--stdio".to_owned()].as_slice()));
        assert!(named(&specs, "rust-analyzer").is_some());
    }
}
