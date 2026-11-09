//! Lists what discovery finds in this machine's real `~/.claude`, to compare
//! with the `skills` / `agents` arrays of Claude Code's own `init` line.
//! Run with `cargo test -p superfik-plugins --test real_home -- --ignored --nocapture`.

use std::path::Path;

use superfik_plugins::{AgentCatalog, AgentDiscovery, SkillCatalog, SkillDiscovery};

#[test]
#[ignore = "reads the real ~/.claude"]
fn lists_real_skills_and_agents() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let skills = SkillDiscovery::new().skills(&repo);
    println!("skills: {:?}", skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>());
    let agents = AgentDiscovery::new().agents(&repo);
    println!("agents: {:?}", agents.iter().map(|a| a.name.as_str()).collect::<Vec<_>>());
    let mut names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), skills.len(), "no duplicates");
}
