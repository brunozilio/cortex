use std::path::{Path, PathBuf};

use serde_json::Value;

use super::{Access, Outcome, Tool, Workspace};
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
    pub fn read(value: &Value) -> Result<Self, String> {
        let mut access = Self::default();
        for (key, slot) in
            [("files", &mut access.files), ("processes", &mut access.processes), ("network", &mut access.network)]
        {
            if let Some(flag) = value.get(key) {
                *slot = flag.as_bool().ok_or_else(|| format!("scripts.{key} must be true or false"))?;
            }
        }
        Ok(access)
    }
}

#[derive(Debug, Clone)]
pub struct ScriptTool {
    pub spec: ToolSpec,
    pub access: Access,
    pub source: String,
    pub path: PathBuf,
}

fn access_named(name: Option<&str>) -> Access {
    match name {
        Some("read") => Access::Reads,
        Some("edit") => Access::Edits,
        _ => Access::Executes,
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
    let access = if trusted { access_named(spec["access"].as_str()) } else { Access::Executes };
    Ok(ScriptTool {
        spec: ToolSpec {
            name: name.to_owned(),
            description: spec["description"].as_str().unwrap_or_default().to_owned(),
            schema: spec.get("input").cloned().unwrap_or_else(|| serde_json::json!({ "type": "object" })),
        },
        access,
        source,
        path: path.to_path_buf(),
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

pub fn load(home: Option<&Path>, cwd: &Path) -> Vec<ScriptTool> {
    let mut folders: Vec<(PathBuf, bool)> =
        home.map(|home| (home.join(brand::DIR).join("tools"), true)).into_iter().collect();
    folders.push((cwd.join(brand::DIR).join("tools"), false));
    let mut tools: Vec<ScriptTool> = Vec::new();
    for (folder, trusted) in folders {
        for file in files(&folder) {
            match parse(&file, trusted) {
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

impl Tool for ScriptTool {
    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn access(&self) -> Access {
        self.access
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
        let tools = load(Some(home.path()), project.path());
        let named: Vec<(&str, Access)> = tools.iter().map(|tool| (tool.spec.name.as_str(), tool.access)).collect();
        assert_eq!(named, [("Echo", Access::Reads), ("Deploy", Access::Executes)]);
        assert_eq!(tools[0].spec.description, "Says it back.");
    }

    #[test]
    fn access_flags_default_to_on_and_can_be_turned_off() {
        assert_eq!(ScriptAccess::read(&serde_json::json!({})), Ok(ScriptAccess::default()));
        let off = ScriptAccess::read(&serde_json::json!({ "network": false })).expect("read");
        assert!(off.files && off.processes && !off.network);
        assert!(ScriptAccess::read(&serde_json::json!({ "files": "no" })).is_err());
    }
}
