use std::path::{Path, PathBuf};

use serde_json::Value;

use super::names::ToolName;
use super::{Declared, Effect, Outcome, Tool, Workspace};
use crate::model::ToolSpec;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScriptAccess {
    pub files: bool,
    pub processes: bool,
    pub network: bool,
}

impl Default for ScriptAccess {
    fn default() -> Self {
        Self { files: true, processes: true, network: true }
    }
}

impl ScriptAccess {
    pub fn none() -> Self {
        Self { files: false, processes: false, network: false }
    }

    pub fn read(value: &Value) -> Result<Self, String> {
        Self::default().layered(value, true)
    }

    pub fn layered(self, value: &Value, owners: bool) -> Result<Self, String> {
        let mut access = self;
        for (key, slot) in
            [("files", &mut access.files), ("processes", &mut access.processes), ("network", &mut access.network)]
        {
            if let Some(flag) = value.get(key) {
                let flag = flag.as_bool().ok_or_else(|| format!("scripts.{key} must be true or false"))?;
                *slot = if owners { flag } else { *slot && flag };
            }
        }
        Ok(access)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Owner,
    Project,
}

impl Origin {
    pub fn trusted(trusted: bool) -> Self {
        if trusted { Self::Owner } else { Self::Project }
    }

    pub fn of_script(shown: &str, home: Option<&Path>) -> Self {
        let owned = |home: &Path| {
            let folder = home.join(brand::DIR);
            let real = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            real(Path::new(shown)).starts_with(real(&folder))
        };
        Self::trusted(shown.starts_with(crate::hooks::BUILTIN_SCRIPT) || home.is_some_and(owned))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScriptGrants {
    pub owner: ScriptAccess,
    #[serde(default = "ScriptAccess::none")]
    pub project: ScriptAccess,
}

impl Default for ScriptGrants {
    fn default() -> Self {
        Self { owner: ScriptAccess::default(), project: ScriptAccess::none() }
    }
}

impl ScriptGrants {
    pub fn none() -> Self {
        Self { owner: ScriptAccess::none(), project: ScriptAccess::none() }
    }

    pub fn of(&self, origin: Origin) -> ScriptAccess {
        match origin {
            Origin::Owner => self.owner,
            Origin::Project => self.project,
        }
    }

    pub fn layered(self, value: &Value, owners: bool) -> Result<Self, String> {
        Ok(Self { owner: self.owner.layered(value, owners)?, project: self.project.layered(value, owners)? })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capability {
    ReadFile,
    WriteFile,
    Exec,
    Fetch,
}

impl Capability {
    pub const ALL: [Self; 4] = [Self::ReadFile, Self::WriteFile, Self::Exec, Self::Fetch];

    pub fn name(self) -> &'static str {
        match self {
            Self::ReadFile => "readFile",
            Self::WriteFile => "writeFile",
            Self::Exec => "exec",
            Self::Fetch => "fetch",
        }
    }

    pub fn of(function: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|capability| capability.name() == function)
    }

    pub fn tool(self) -> &'static str {
        match self {
            Self::ReadFile => ToolName::Read.as_str(),
            Self::WriteFile => ToolName::Write.as_str(),
            Self::Exec => ToolName::Bash.as_str(),
            Self::Fetch => ToolName::WebFetch.as_str(),
        }
    }

    pub fn subject(self, arguments: &Value) -> String {
        let text = |key: &str| arguments.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
        match self {
            Self::ReadFile => arguments.as_str().unwrap_or_default().to_owned(),
            Self::WriteFile => text("path"),
            Self::Exec => text("command"),
            Self::Fetch => text("url"),
        }
    }

    pub fn effects(self, arguments: &Value, place: &dyn Fn(&str) -> domain::permission::Place) -> Vec<Effect> {
        let subject = self.subject(arguments);
        match self {
            Self::ReadFile => vec![Effect::ReadPath(place(&subject))],
            Self::WriteFile => vec![Effect::WritePath(place(&subject))],
            Self::Exec => vec![Effect::exec(&subject, false)],
            Self::Fetch => vec![Effect::Net { host: domain::permission_rules::site(&subject) }],
        }
    }

    pub fn granted(self, access: ScriptAccess) -> Result<(), String> {
        let (granted, setting) = match self {
            Self::ReadFile | Self::WriteFile => {
                (access.files, "scripts may not touch files here (settings: scripts.files)")
            }
            Self::Exec => (access.processes, "scripts may not start processes here (settings: scripts.processes)"),
            Self::Fetch => (access.network, "scripts may not reach the network here (settings: scripts.network)"),
        };
        if granted { Ok(()) } else { Err(setting.to_owned()) }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Declaration {
    #[default]
    Spec,
    Export,
    ServerExport,
    Nowhere(&'static str),
}

impl Declaration {
    fn place(self) -> &'static str {
        match self {
            Self::Spec => "spec.capabilities",
            Self::Export => "export const capabilities = [...]",
            Self::ServerExport => "the capabilities list of export default { capabilities: [...], tools }",
            Self::Nowhere(_) => "",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Capabilities {
    listed: Vec<Capability>,
    declared_in: Declaration,
}

impl Capabilities {
    pub fn read(spec: &Value) -> Result<Self, String> {
        Self::listed(spec.get("capabilities"), Declaration::Spec)
    }

    pub fn listed(listed: Option<&Value>, declared_in: Declaration) -> Result<Self, String> {
        let place = declared_in.place();
        let Some(listed) = listed.filter(|listed| !listed.is_null()) else {
            return Ok(Self { listed: Vec::new(), declared_in });
        };
        let names = listed.as_array().ok_or_else(|| format!("{place} must be a list"))?;
        let known = || Capability::ALL.map(Capability::name).join(", ");
        let read = names.iter().map(|name| {
            let name = name.as_str().unwrap_or_default();
            Capability::of(name).ok_or_else(|| format!("{place}: {name:?} is not one of {}", known()))
        });
        let listed = read.collect::<Result<Vec<_>, _>>()?;
        if let Declaration::Nowhere(what) = declared_in
            && !listed.is_empty()
        {
            return Err(format!("{what} cannot declare capabilities: it has no host functions"));
        }
        Ok(Self { listed, declared_in })
    }

    pub fn none_in(what: &'static str) -> Self {
        Self { listed: Vec::new(), declared_in: Declaration::Nowhere(what) }
    }

    pub fn declares(&self, capability: Capability) -> bool {
        self.listed.contains(&capability)
    }

    pub fn allows(&self, capability: Capability, script: &str) -> Result<(), String> {
        if self.declares(capability) {
            return Ok(());
        }
        let name = capability.name();
        if let Declaration::Nowhere(what) = self.declared_in {
            return Err(format!("{what} cannot call ctx.{name}: it has no host functions"));
        }
        let place = self.declared_in.place();
        Err(format!(
            "{script} did not declare ctx.{name}: add \"{name}\" to {place} to use it (it is still asked about like \
             any tool call)"
        ))
    }
}

#[derive(Debug, Clone)]
pub struct ScriptTool {
    pub spec: ToolSpec,
    pub declared: Option<Declared>,
    pub capabilities: Capabilities,
    pub source: String,
    pub path: PathBuf,
    pub trusted: bool,
}

fn declared_by(access: Option<&str>) -> Option<Declared> {
    match access {
        Some("read") => None,
        Some("edit") => Some(Declared::Edits),
        _ => Some(Declared::Executes),
    }
}

fn parse(path: &Path, trusted: bool) -> Result<ScriptTool, String> {
    let source = std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let shown = path.display().to_string();
    let spec = script::exported(&shown, &source, "spec").map_err(|error| format!("{shown}: {error}"))?;
    let name = spec["name"]
        .as_str()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("{shown}: spec.name is missing"))?;
    let declared = if trusted { declared_by(spec["access"].as_str()) } else { Some(Declared::Executes) };
    let capabilities = Capabilities::read(&spec).map_err(|error| format!("{shown}: {error}"))?;
    Ok(ScriptTool {
        capabilities,
        spec: ToolSpec {
            name: name.to_owned(),
            description: spec["description"].as_str().unwrap_or_default().to_owned(),
            schema: spec.get("input").cloned().unwrap_or_else(|| serde_json::json!({ "type": "object" })),
        },
        declared,
        source,
        path: path.to_path_buf(),
        trusted,
    })
}

fn files(folder: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter_map(|path| {
            if path.is_dir() {
                Some(path.join("tool.js")).filter(|file| file.is_file())
            } else {
                path.extension().is_some_and(|ext| ext == "js").then_some(path)
            }
        })
        .collect();
    found.sort();
    found
}

pub fn load(gate: &crate::trust::Gate) -> Vec<ScriptTool> {
    let mut folders: Vec<(PathBuf, bool)> =
        gate.home().map(|home| (home.join(brand::DIR).join("tools"), true)).into_iter().collect();
    let project = gate.cwd().join(brand::DIR).join("tools");
    if gate.honours(&project) {
        folders.push((project, false));
    }
    let mut tools: Vec<ScriptTool> = Vec::new();
    let built_in = |file: &Path| {
        file.file_name().is_some_and(|name| content::builtin(&format!("tools/{}", name.to_string_lossy())).is_some())
    };
    for (folder, trusted) in folders {
        for file in files(&folder).into_iter().filter(|file| !built_in(file)) {
            match parse(&file, trusted) {
                Ok(tool) if !trusted && tools.iter().any(|known| known.spec.name == tool.spec.name) => {
                    tracing::warn!(name = %tool.spec.name, "a project script tool with the owner's tool's name is left out")
                }
                Ok(tool) => {
                    tools.retain(|known| known.spec.name != tool.spec.name);
                    tools.push(tool);
                }
                Err(error) => tracing::warn!(%error, "a script tool is left out"),
            }
        }
    }
    tools
}

pub fn join(tools: &mut Vec<Box<dyn Tool>>, scripts: Vec<ScriptTool>) {
    for script in scripts {
        let taken = tools.iter().any(|tool| tool.spec().name == script.spec.name);
        if taken && !script.trusted {
            tracing::warn!(name = %script.spec.name, "a project script tool with an existing tool's name is left out");
            continue;
        }
        tools.retain(|tool| tool.spec().name != script.spec.name);
        tools.push(Box::new(script));
    }
}

impl Tool for ScriptTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn effects(&self, _input: &Value, _workspace: &Workspace) -> Vec<Effect> {
        self.declared.map(Effect::Declared).into_iter().collect()
    }

    fn summary(&self, input: &Value) -> String {
        input.as_object().and_then(|fields| fields.values().find_map(Value::as_str)).unwrap_or_default().to_owned()
    }

    fn run(&self, _input: &Value, _workspace: &Workspace) -> Outcome {
        Outcome::error(format!("{} runs only inside an agent", self.spec.name))
    }

    fn script(&self) -> Option<&ScriptTool> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_come_from_the_user_s_and_the_project_s_folders_and_project_ones_always_ask() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        let user_tools = home.path().join(brand::DIR).join("tools");
        let project_tools = project.path().join(brand::DIR).join("tools").join("deploy");
        std::fs::create_dir_all(&user_tools).expect("dir");
        std::fs::create_dir_all(&project_tools).expect("dir");
        std::fs::write(
            user_tools.join("echo.js"),
            r#"export const spec = { name: "Echo", description: "Says it back.", access: "read" }; export function run(i) { return i }"#,
        )
        .expect("write");
        std::fs::write(project_tools.join("tool.js"), r#"export const spec = { name: "Deploy", access: "read" };"#)
            .expect("write");
        std::fs::write(user_tools.join("broken.js"), "export const spec = ").expect("write");
        let tools = load(&crate::trust::trusted(project.path(), home.path()));
        let named: Vec<(&str, Option<Declared>)> =
            tools.iter().map(|tool| (tool.spec.name.as_str(), tool.declared)).collect();
        assert_eq!(named, [("Echo", None), ("Deploy", Some(Declared::Executes))]);
        assert_eq!(tools[0].spec.description, "Says it back.");
        let untrusted = load(&crate::trust::untrusted(project.path(), home.path()));
        let named: Vec<&str> = untrusted.iter().map(|tool| tool.spec.name.as_str()).collect();
        assert_eq!(named, ["Echo"], "an untrusted folder's tools are not registered");
    }

    #[test]
    fn a_script_declares_the_host_functions_it_uses_and_nothing_else_is_allowed() {
        let none = Capabilities::read(&serde_json::json!({ "name": "T" })).expect("read");
        assert!(Capability::ALL.into_iter().all(|capability| !none.declares(capability)));
        let some = Capabilities::read(&serde_json::json!({ "capabilities": ["exec", "readFile"] })).expect("read");
        assert!(some.declares(Capability::Exec) && some.declares(Capability::ReadFile));
        assert!(!some.declares(Capability::WriteFile) && !some.declares(Capability::Fetch));
        let refused = some.allows(Capability::Fetch, "Runner").expect_err("undeclared");
        assert!(refused.contains("Runner") && refused.contains("\"fetch\""), "{refused}");
        assert!(Capabilities::read(&serde_json::json!({ "capabilities": ["root"] })).is_err());
        assert!(Capabilities::read(&serde_json::json!({ "capabilities": "exec" })).is_err());
        assert_eq!(Capability::of("exec"), Some(Capability::Exec));
        assert_eq!(Capability::of("log"), None);
    }

    #[test]
    fn each_kind_of_script_is_told_where_to_declare_what_it_uses() {
        let exported = Capabilities::listed(Some(&serde_json::json!(["exec"])), Declaration::Export).expect("read");
        assert!(exported.declares(Capability::Exec));
        let refused = exported.allows(Capability::WriteFile, "the hook guard.js").expect_err("undeclared");
        assert!(
            refused.contains("export const capabilities = [...]") && refused.contains("\"writeFile\""),
            "{refused}"
        );
        let server = Capabilities::listed(None, Declaration::ServerExport).expect("read");
        let refused = server.allows(Capability::Fetch, "the MCP server docs").expect_err("undeclared");
        assert!(refused.contains("export default { capabilities"), "{refused}");
        assert!(Capabilities::listed(Some(&serde_json::json!("exec")), Declaration::Export).is_err());
        let provider = Capabilities::none_in("a provider's shape");
        let refused = provider.allows(Capability::Exec, "x").expect_err("nothing");
        assert!(refused.contains("a provider's shape cannot call ctx.exec"), "{refused}");
        let listed = Capabilities::listed(Some(&serde_json::json!(["exec"])), Declaration::Nowhere("a provider"));
        assert!(listed.is_err_and(|why| why.contains("a provider cannot declare capabilities")));
    }

    #[test]
    fn a_host_call_names_the_tool_it_is_judged_as_and_its_subject() {
        let place = |path: &str| domain::permission::Place::at(path);
        assert_eq!(Capability::Exec.tool(), "Bash");
        assert_eq!(Capability::Fetch.subject(&serde_json::json!({ "url": "https://x.dev" })), "https://x.dev");
        let effects = Capability::WriteFile.effects(&serde_json::json!({ "path": "/a.txt" }), &place);
        assert_eq!(effects, [Effect::WritePath(place("/a.txt"))]);
    }

    #[test]
    fn access_flags_default_to_on_and_can_be_turned_off() {
        assert_eq!(ScriptAccess::read(&serde_json::json!({})), Ok(ScriptAccess::default()));
        let off = ScriptAccess::read(&serde_json::json!({ "network": false })).expect("read");
        assert!(off.files && off.processes && !off.network);
        assert!(ScriptAccess::read(&serde_json::json!({ "files": "no" })).is_err());
    }
}

#[cfg(test)]
mod join_tests {
    use super::*;

    #[test]
    fn a_project_s_tool_never_takes_the_name_of_a_tool_that_is_already_there() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        let user_tools = home.path().join(brand::DIR).join("tools");
        let project_tools = project.path().join(brand::DIR).join("tools");
        std::fs::create_dir_all(&user_tools).expect("dir");
        std::fs::create_dir_all(&project_tools).expect("dir");
        let spec = |name: &str, description: &str| {
            format!(r#"export const spec = {{ name: "{name}", description: "{description}" }};"#)
        };
        std::fs::write(user_tools.join("echo.js"), spec("Echo", "The owner's.")).expect("write");
        std::fs::write(user_tools.join("grep.js"), spec("Grep", "The owner's.")).expect("write");
        std::fs::write(project_tools.join("bash.js"), spec("Bash", "The project's.")).expect("write");
        std::fs::write(project_tools.join("echo.js"), spec("Echo", "The project's.")).expect("write");
        std::fs::write(project_tools.join("deploy.js"), spec("Deploy", "The project's.")).expect("write");

        let mut tools = crate::tools::standard();
        join(&mut tools, load(&crate::trust::trusted(project.path(), home.path())));

        let described = |name: &str| -> Vec<(String, bool)> {
            tools
                .iter()
                .filter(|tool| tool.spec().name == name)
                .map(|tool| (tool.spec().description, tool.script().is_some()))
                .collect()
        };
        let bash = described("Bash");
        assert_eq!(bash.len(), 1);
        assert!(!bash[0].1, "the built-in Bash stays: {bash:?}");
        assert_eq!(described("Echo"), [("The owner's.".to_owned(), true)]);
        assert_eq!(described("Grep"), [("The owner's.".to_owned(), true)], "the owner may replace a built-in");
        assert_eq!(described("Deploy"), [("The project's.".to_owned(), true)]);
    }
}

#[cfg(test)]
mod copied_content_tests {
    use super::*;

    #[test]
    fn a_copy_of_a_built_in_tool_in_the_user_s_folder_is_not_a_script_tool() {
        let home = tempfile::tempdir().expect("home");
        let project = tempfile::tempdir().expect("project");
        let user_tools = home.path().join(brand::DIR).join("tools");
        std::fs::create_dir_all(&user_tools).expect("dir");
        let bash = content::builtin("tools/Bash.js").expect("Bash ships");
        std::fs::write(user_tools.join("Bash.js"), bash).expect("copy");
        std::fs::write(user_tools.join("echo.js"), r#"export const spec = { name: "Echo" };"#).expect("write");
        let names: Vec<String> =
            load(&crate::trust::trusted(project.path(), home.path())).into_iter().map(|tool| tool.spec.name).collect();
        assert_eq!(names, ["Echo"]);
    }
}
