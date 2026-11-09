//! Discovery tests against temporary skill directories laid out like a
//! real Claude Code home.

use super::*;

fn write_skill(skills_dir: &Path, folder: &str, file: &str, text: &str) {
    let dir = skills_dir.join(folder);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(dir.join(file), text).expect("write");
}

fn names(skills: &[Skill]) -> Vec<&str> {
    skills.iter().map(|skill| skill.name.as_str()).collect()
}

#[test]
fn discovers_repo_then_user_skills_with_repo_precedence() {
    let repo = tempfile::tempdir().expect("repo");
    let home = tempfile::tempdir().expect("home");
    let repo_skills = repo.path().join(".claude/skills");
    let user_skills = home.path().join("skills");
    write_skill(&repo_skills, "motion", "SKILL.md", "---\nname: motion\ndescription: Repo motion\n---\n");
    write_skill(&repo_skills, "dither-minis", "SKILL.md", "---\ndescription: Tiny dithers\n---\n");
    write_skill(&user_skills, "motion", "SKILL.md", "---\nname: motion\ndescription: User motion\n---\n");
    write_skill(&user_skills, "skeuomorphic-ui", "SKill.md", "---\nname: skeuomorphic-ui\n---\n# UI\n\nDark knobs.\n");
    write_skill(&user_skills, "no-file", "README.md", "nothing");

    let skills = SkillDiscovery::with_home(Some(home.path().to_path_buf())).skills(repo.path());
    assert_eq!(names(&skills), vec!["dither-minis", "motion", "skeuomorphic-ui"]);
    assert_eq!(skills[0].title, "Dither minis skill");
    assert_eq!(skills[0].description, "Tiny dithers");
    assert_eq!(skills[1].description, "Repo motion");
    assert_eq!(skills[2].description, "Dark knobs.");
    assert_eq!(skills[2].slash(), "/skeuomorphic-ui");
}

#[test]
fn synced_and_plugin_skills_are_namespaced_and_lose_clashes() {
    let repo = tempfile::tempdir().expect("repo");
    let home = tempfile::tempdir().expect("home");
    write_skill(&home.path().join("skills"), "pdf", "SKILL.md", "---\nname: pdf\ndescription: Mine\n---\n");
    let synced = home.path().join("skills/synced/bucket-1");
    write_skill(&synced, "pdf", "SKILL.md", "---\nname: pdf\ndescription: Synced\n---\n");
    write_skill(&synced, "docx", "SKILL.md", "---\nname: docx\ndescription: Word\n---\n");
    let plugin = home.path().join("plugins/cache/official/review/1.0.0");
    write_skill(&plugin.join("skills"), "review-pr", "SKILL.md", "---\nname: review-pr\ndescription: PRs\n---\n");
    let off = home.path().join("plugins/cache/official/off/1.0.0");
    write_skill(&off.join("skills"), "hidden", "SKILL.md", "---\nname: hidden\ndescription: Off\n---\n");
    let catalogue = home.path().join("plugins/marketplaces/official/plugins/other/skills");
    write_skill(&catalogue, "not-installed", "SKILL.md", "---\nname: not-installed\ndescription: x\n---\n");
    let installed = format!(
        r#"{{"version": 2, "plugins": {{
            "review@official": [{{"scope": "user", "installPath": {}}}],
            "off@official": [{{"scope": "user", "installPath": {}}}]}}}}"#,
        serde_json::json!(plugin),
        serde_json::json!(off)
    );
    fs::write(home.path().join("plugins/installed_plugins.json"), installed).expect("write");
    fs::write(home.path().join("settings.json"), r#"{"enabledPlugins": {"off@official": false}}"#).expect("write");

    let skills = SkillDiscovery::with_home(Some(home.path().to_path_buf())).skills(repo.path());
    assert_eq!(names(&skills), vec!["pdf", "anthropic-skills:docx", "review:review-pr"]);
    assert_eq!(skills[0].description, "Mine");
    assert_eq!(skills[1].title, "Docx skill");
    assert_eq!(skills[2].slash(), "/review:review-pr");
}

#[test]
fn missing_directories_yield_nothing() {
    let repo = tempfile::tempdir().expect("repo");
    let discovery = SkillDiscovery::with_home(Some(repo.path().join("nope")));
    assert!(discovery.skills(repo.path()).is_empty());
    assert!(SkillDiscovery::with_home(None).skills(Path::new("/definitely/not/here")).is_empty());
}

#[test]
fn skill_from_markdown_falls_back_to_folder_name() {
    let skill = skill_from_markdown("pdf-tools", "no frontmatter\n");
    assert_eq!(skill.name, "pdf-tools");
    assert_eq!(skill.title, "Pdf tools skill");
    assert_eq!(skill.description, "no frontmatter");
    assert_eq!(skill.icon, skill_icon("pdf-tools"));
}
