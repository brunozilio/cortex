#[test]
fn the_chat_s_review_and_init_commands_ship_as_skills() {
    for name in ["init", "review", "code-review", "security-review"] {
        let path = format!("{name}/SKILL.md");
        let skill = content::builtin(&format!("skills/{path}"));
        let text = String::from_utf8_lossy(skill.unwrap_or_default());
        assert!(text.contains(&format!("name: {name}\n")), "{path}: {text}");
    }
}

#[test]
fn the_agent_may_load_the_configure_skill_to_set_up_model_fallback() {
    let skill = content::builtin("skills/configure/SKILL.md").expect("configure ships");
    let text = String::from_utf8_lossy(skill);
    assert!(text.contains("name: configure\n"), "{text}");
    assert!(!text.contains("disable-model-invocation"), "the model may load it on its own");
    for key in [
        "fallbackModel",
        "ModelRequest",
        "ModelError",
        "provider:model",
        "settings.local.json",
        "mcp.json",
        "{{COMMAND}} mcp add",
        "\"type\": \"http\"",
        "enabledMcpServers",
        "{{COMMAND}} mcp login",
        "-H 'Authorization: Bearer ${",
        "register themselves",
    ] {
        assert!(text.contains(key), "configure explains {key}");
    }
}

#[test]
fn every_shipped_skill_speaks_as_brand() {
    for (path, bytes) in content::builtin_files("skills/") {
        if path.ends_with("LICENSE.txt") || !path.ends_with(".md") {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        assert!(!brand::text(&text).contains("{{"), "{path}: an unknown placeholder");
        for other in ["Claude", "Anthropic", "claude.ai"] {
            assert!(!text.contains(other), "{path} names {other}");
        }
    }
}
