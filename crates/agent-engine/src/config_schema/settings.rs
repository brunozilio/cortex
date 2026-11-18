use serde_json::{Map, Value, json};

use super::{DRAFT, list_of, map_of, reference, reference_to};

pub fn schema() -> Value {
    json!({
        "$schema": DRAFT,
        "title": "Settings",
        "description": "A settings.json, settings.local.json or managed-settings.json file of the app.",
        "type": "object",
        "properties": properties(),
        "additionalProperties": false,
        "$defs": { "hookGroup": hook_group(), "hook": hook(), "languageServer": language_server() }
    })
}

fn model(about: &str) -> Value {
    json!({ "type": ["string", "null"], "description": about })
}

fn names(about: &str) -> Value {
    json!({ "type": "array", "items": { "type": "string" }, "description": about })
}

fn switch(about: &str) -> Value {
    json!({ "type": "boolean", "description": about })
}

fn properties() -> Value {
    json!({
        "$schema": reference(),
        "outputStyle": {
            "type": "string",
            "description": "How the agent talks: a built-in style (default, concise, proactive, explanatory, learning) \
                            or the name of a custom style in output-styles/<name>.md."
        },
        "agent": { "type": ["string", "null"], "description": "Runs the main conversation as this agent." },
        "bashOutputLimit": {
            "type": "integer",
            "minimum": domain::output::SMALLEST_LIMIT,
            "maximum": domain::output::LARGEST_LIMIT,
            "default": domain::output::DEFAULT_LIMIT,
            "description": "Characters of a command's output the agent sees; past it, the start and the end are kept \
                            and the whole output is saved to a file the agent is pointed to."
        },
        "advisorModel": model("Model for the advisor the agent consults."),
        "judgeModel": model("Model for the judge that checks finished work."),
        "smallModel": model("Model for background chores: titles, effort picks, web reading."),
        "teammateMode": {
            "enum": ["process", "tmux", "iterm2", "auto"],
            "default": "process",
            "description": "Where a lead starts its teammates: process (in the background, printing to a log), tmux (in a new tmux pane of the window the lead runs in), iterm2 (in a new iTerm2 pane beside the lead) or auto (tmux inside tmux, else iTerm2 inside iTerm2, else process)."
        },
        "escalationModel": model("Where a call the agent keeps failing is sent. Read only from your own files."),
        "fallbackModel": {
            "type": ["string", "array", "null"],
            "items": { "type": ["string", "null"] },
            "description": "Model, or chain of models, tried when the chosen one is unavailable."
        },
        "jsonSchema": {
            "type": ["object", "boolean", "null"],
            "description": "Gives the agent a StructuredOutput tool whose result must match this JSON Schema."
        },
        "enabledProviders": names("Ids of providers declared by the project that you allow to load."),
        "enableAllProjectMcpServers": switch("Starts every MCP server the project's mcp.json declares without asking."),
        "enabledMcpServers": names("Project MCP servers you approved."),
        "disabledMcpServers": names("Project MCP servers you turned off; wins over the approvals."),
        "hooks": hooks(),
        "lspServers": map_of(reference_to("languageServer"), "Adds, changes or disables language servers, by name."),
        "permissions": permissions(),
        "verification": {
            "type": "object",
            "properties": { "allow": names("Rules the verification judge may run without asking.") },
            "additionalProperties": false
        },
        "scripts": {
            "type": "object",
            "description": "What JavaScript tools, skills, hooks and providers may do.",
            "properties": {
                "files": switch("Scripts may read and write files."),
                "processes": switch("Scripts may start processes."),
                "network": switch("Scripts may reach the network.")
            },
            "additionalProperties": false
        },
        "sandbox": {
            "type": "object",
            "description": "Where sandboxed commands may write and whether they reach the network.",
            "properties": {
                "network": switch("false blocks the network for sandboxed commands."),
                "required": switch("true refuses commands when the sandbox cannot be provided."),
                "allowWrite": names("Extra folders sandboxed commands may write to."),
                "allowUnixSockets": names(
                    "Service sockets sandboxed commands may connect to on macOS, such as the Docker socket or a tmux server's (refused by default)."
                ),
                "allowRead": names(
                    "Paths sandboxed commands may read on macOS although they are credential stores (~/.ssh, ~/.aws, ~/.npmrc, ~/.gnupg, …) or the app's data folder, which are unreadable by default."
                )
            },
            "additionalProperties": false
        },
        "features": features(),
        domain::trust::LEGACY_OPT_IN: {
            "type": "boolean",
            "deprecated": true,
            "description": "No longer does anything: a project's hooks run once you trust its folder."
        }
    })
}

fn permissions() -> Value {
    json!({
        "type": "object",
        "description": "Allow, ask and deny rules and extra folders.",
        "properties": {
            "allow": names("Tool calls that run without asking. Read only from your own files."),
            "ask": names("Tool calls that always ask."),
            "deny": names("Tool calls that are refused, in every mode."),
            "additionalDirectories": names("Folders the agent reads like the project. Read only from your own files.")
        },
        "additionalProperties": false
    })
}

fn features() -> Value {
    let known: Map<String, Value> =
        domain::features::every().map(|flag| (flag.name.to_owned(), switch(&unbranded(flag.about)))).collect();
    json!({
        "type": "object",
        "description": "Turns features on or off by name.",
        "properties": known,
        "additionalProperties": { "type": "boolean" }
    })
}

fn unbranded(text: &str) -> String {
    text.replace(brand::DIR, "{{DIR}}")
}

fn language_server() -> Value {
    let text = json!({ "type": "string" });
    json!({
        "type": "object",
        "properties": {
            "command": { "type": ["string", "null"], "description": "Program to start." },
            "args": { "type": ["array", "null"], "items": text },
            "extensionToLanguage": { "type": ["object", "null"], "additionalProperties": text },
            "rootMarkers": { "type": ["array", "null"], "items": text },
            "env": { "type": ["object", "null"], "additionalProperties": text },
            "disabled": switch("true removes the server.")
        },
        "additionalProperties": false
    })
}

fn hooks() -> Value {
    let groups = list_of(reference_to("hookGroup"));
    let events: Map<String, Value> =
        crate::hooks::HookEvent::ALL.iter().map(|event| (event.as_str().to_owned(), groups.clone())).collect();
    json!({
        "type": "object",
        "description": "Commands, prompts or HTTP calls that run on events.",
        "properties": events,
        "additionalProperties": false
    })
}

fn hook_group() -> Value {
    json!({
        "type": "object",
        "properties": {
            "matcher": { "type": "string", "description": "A regex over the subject; empty or * matches all." },
            "hooks": list_of(reference_to("hook"))
        },
        "required": ["hooks"],
        "additionalProperties": false
    })
}

fn hook() -> Value {
    let text = json!({ "type": "string" });
    let (shortest, longest) = (crate::hooks::TIMEOUT_SECONDS.start(), crate::hooks::TIMEOUT_SECONDS.end());
    json!({
        "type": "object",
        "properties": {
            "type": { "enum": crate::hooks::HandlerKind::ALL.map(crate::hooks::HandlerKind::as_str) },
            "command": text,
            "prompt": { "type": ["string", "null"] },
            "model": { "type": ["string", "null"] },
            "url": { "type": ["string", "null"] },
            "headers": map_of(text.clone(), "Request headers of an http hook."),
            "allowed_env_vars": list_of(text.clone()),
            "timeout": { "type": "integer", "minimum": shortest, "maximum": longest, "description": "Seconds." },
            "status_message": { "description": "Accepted; not shown." }
        },
        "required": ["type"],
        "additionalProperties": false
    })
}
