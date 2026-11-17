use std::path::{Path, PathBuf};

use domain::trust::{ContributionKind as Kind, Contributions};
use serde_json::Value;

use super::Gate;

use crate::tools::subagent::PRIVILEGES as AGENT_PRIVILEGES;

pub fn contributions(gate: &Gate) -> Contributions {
    let mut found = Contributions::default();
    let folders = gate.project_folders();
    let shown = |path: &Path| path.strip_prefix(gate.root()).unwrap_or(path).display().to_string();
    for folder in &folders {
        settings_of(folder, &shown, &mut found);
        for script in files_in(&folder.join("hooks"), "js") {
            found.add(Kind::HookScript, shown(&script));
        }
        for script in skill_scripts(folder) {
            found.add(Kind::SkillScript, shown(&script));
        }
        for (agent, keys) in privileged_agents(folder) {
            found.add(Kind::AgentPrivilege, format!("{}: {}", shown(&agent), keys.join(", ")));
        }
    }
    if let Some(config) = folders.iter().map(|folder| folder.join("mcp.json")).find(|path| path.is_file()) {
        for server in mcp_servers(&config) {
            found.add(Kind::McpServer, server);
        }
    }
    if let Some(own) = folders.first().filter(|folder| folder.parent().is_some_and(|dir| dir == settled_cwd(gate))) {
        for script in files_in(&own.join("mcp"), "js") {
            found.add(Kind::McpServer, format!("{} (script)", shown(&script)));
        }
        for script in files_in(&own.join("providers"), "js") {
            found.add(Kind::DeclaredProvider, shown(&script));
        }
        for script in tool_scripts(&own.join("tools")) {
            found.add(Kind::ScriptTool, shown(&script));
        }
    }
    found
}

fn settled_cwd(gate: &Gate) -> PathBuf {
    crate::tools::settled(gate.cwd())
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

fn entries(folder: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> =
        std::fs::read_dir(folder).into_iter().flatten().flatten().map(|entry| entry.path()).collect();
    found.sort();
    found
}

fn files_in(folder: &Path, extension: &str) -> Vec<PathBuf> {
    let wanted = |path: &PathBuf| path.is_file() && path.extension().is_some_and(|ext| ext == extension);
    entries(folder).into_iter().filter(wanted).collect()
}

fn tool_scripts(folder: &Path) -> Vec<PathBuf> {
    let tool = |path: PathBuf| if path.is_dir() { path.join("tool.js") } else { path };
    let script = |path: &PathBuf| path.is_file() && path.extension().is_some_and(|ext| ext == "js");
    entries(folder).into_iter().map(tool).filter(script).collect()
}

fn skill_scripts(folder: &Path) -> Vec<PathBuf> {
    let named_skill = |path: &PathBuf| path.file_name().is_some_and(|name| name.eq_ignore_ascii_case("skill.js"));
    let skills = entries(&folder.join("skills"))
        .into_iter()
        .filter(|path| path.is_dir())
        .flat_map(|skill| files_in(&skill, "js"))
        .filter(named_skill);
    let commands = ignore::WalkBuilder::new(folder.join("commands"))
        .hidden(false)
        .max_depth(Some(8))
        .build()
        .filter_map(Result::ok)
        .map(ignore::DirEntry::into_path)
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "js"));
    skills.chain(commands).collect()
}

fn privileged_agents(folder: &Path) -> Vec<(PathBuf, Vec<&'static str>)> {
    files_in(&folder.join("agents"), "md")
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let (front, _) = crate::prompt::markdown::frontmatter(&text).ok()?;
            let keys: Vec<&str> = AGENT_PRIVILEGES.into_iter().filter(|key| front.get(key).is_some()).collect();
            (!keys.is_empty()).then_some((path, keys))
        })
        .collect()
}

fn mcp_servers(config: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(config) else { return Vec::new() };
    match crate::mcp::parse_config(&text) {
        Ok(servers) => servers.iter().map(|server| format!("{}: {}", server.name(), server.target())).collect(),
        Err(_) => vec![format!("{} (unreadable)", config.display())],
    }
}

fn settings_of(folder: &Path, shown: &dyn Fn(&Path) -> String, found: &mut Contributions) {
    for name in ["settings.json", "settings.local.json"] {
        let path = folder.join(name);
        let Some(settings) = read_json(&path) else { continue };
        let file = shown(&path);
        if let Some(hooks) = settings.get("hooks") {
            for hook in hooks_in(hooks) {
                found.add(Kind::SettingsHook, format!("{hook} ({file})"));
            }
        }
        if name == "settings.local.json" {
            for grant in grants(&settings) {
                found.add(Kind::LocalSettings, grant);
            }
            for command in lsp_commands(&settings) {
                found.add(Kind::LspCommand, command);
            }
        }
    }
}

fn hooks_in(hooks: &Value) -> Vec<String> {
    match crate::hooks::Hooks::parse(hooks) {
        Ok(hooks) => hooks
            .listed()
            .into_iter()
            .map(|hook| {
                let matcher = if hook.matcher.is_empty() { "*" } else { hook.matcher.as_str() };
                format!("{} {matcher}: {}", hook.event, hook.runs)
            })
            .collect(),
        Err(_) => hooks.as_object().into_iter().flatten().map(|(event, _)| event.clone()).collect(),
    }
}

fn texts(value: &Value) -> impl Iterator<Item = &str> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str)
}

fn grants(settings: &Value) -> Vec<String> {
    let mut grants: Vec<String> = Vec::new();
    let permissions = &settings["permissions"];
    grants.extend(texts(&permissions["allow"]).map(|rule| format!("allow {rule}")));
    grants.extend(texts(&permissions["additionalDirectories"]).map(|dir| format!("additional folder {dir}")));
    grants.extend(texts(&settings["verification"]["allow"]).map(|rule| format!("allow for the judge {rule}")));
    for (key, flag) in settings["scripts"].as_object().into_iter().flatten() {
        if flag.as_bool() == Some(true) {
            grants.push(format!("script access to {key}"));
        }
    }
    for (feature, flag) in settings["features"].as_object().into_iter().flatten() {
        if let Some(flag) = flag.as_bool().filter(|flag| !domain::features::only_restricts(feature, *flag)) {
            grants.push(format!("feature {feature} turned {}", if flag { "on" } else { "off" }));
        }
    }
    if settings["sandbox"]["network"].as_bool() == Some(true) {
        grants.push("sandbox network access".to_owned());
    }
    grants.extend(texts(&settings["sandbox"]["allowWrite"]).map(|dir| format!("sandbox may write to {dir}")));
    grants.extend(
        texts(&settings["sandbox"]["allowUnixSockets"]).map(|socket| format!("sandbox may connect to {socket}")),
    );
    grants.extend(texts(&settings["sandbox"]["allowRead"]).map(|path| format!("sandbox may read {path}")));
    if settings["enableAllProjectMcpServers"].as_bool() == Some(true) {
        grants.push("approval of every project MCP server".to_owned());
    }
    grants.extend(texts(&settings["enabledMcpServers"]).map(|name| format!("approval of the MCP server {name}")));
    grants.extend(texts(&settings["disabledMcpServers"]).map(|name| format!("the MCP server {name} turned off")));
    grants.extend(texts(&settings["enabledProviders"]).map(|id| format!("approval of the provider {id}")));
    if let Some(model) = settings["escalationModel"].as_str() {
        grants.push(format!("escalation model {model}"));
    }
    grants
}

fn lsp_commands(settings: &Value) -> Vec<String> {
    let servers = settings["lspServers"].as_object().into_iter().flatten();
    servers
        .filter_map(|(name, server)| {
            let changed = ["command", "args", "env"].iter().any(|key| server.get(key).is_some());
            let runs = match server["command"].as_str() {
                Some(command) => std::iter::once(command).chain(texts(&server["args"])).collect::<Vec<_>>().join(" "),
                None => "its arguments or environment".to_owned(),
            };
            changed.then(|| format!("{name}: {runs}"))
        })
        .collect()
}
