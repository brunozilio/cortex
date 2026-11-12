use domain::{Extensions, ModelOption};

use super::*;

fn skill(dir: &Path, name: &str, description: &str) {
    std::fs::create_dir_all(dir.join(name)).expect("dir");
    let text = format!("---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\nDo the {name} thing.\n");
    std::fs::write(dir.join(name).join("SKILL.md"), text).expect("write");
}

#[test]
fn finds_the_project_s_the_user_s_and_the_host_s_skills() {
    let repo = tempfile::tempdir().expect("repo");
    let home = tempfile::tempdir().expect("home");
    let plugin = tempfile::tempdir().expect("plugin");
    skill(&repo.path().join(concat!(brand::dir!(), "/skills")), "release", "Cuts a release.");
    skill(&home.path().join(concat!(brand::dir!(), "/skills")), "release", "The user's copy.");
    skill(&home.path().join(concat!(brand::dir!(), "/skills")), "morning", "The morning briefing.");
    skill(&plugin.path().join("skills"), "dataviz", "Charts.");
    let model = ModelOption::new("m", "M");
    let mut request = PromptRequest::plain(repo.path().to_path_buf(), "hi", model);
    request.extensions = Some(Extensions { dirs: vec![plugin.path().to_path_buf()], ..Extensions::default() });
    let found = discover(&request, Some(home.path()));
    let names: Vec<(&str, &str)> = found.iter().map(|s| (s.name.as_str(), s.description.as_str())).collect();
    assert_eq!(
        &names[..3],
        [("release", "Cuts a release."), ("morning", "The morning briefing."), ("dataviz", "Charts.")],
        "the project's, the user's and the host's first"
    );
    assert!(
        names[3..].iter().any(|(name, _)| *name == "code-review"),
        "then {brand_name}'s own",
        brand_name = brand::NAME
    );
    assert_eq!(listing(&found[..1]), "- release: Cuts a release.");

    let tool = SkillTool { skills: found };
    let loaded = tool.run(&json!({ "skill": "/dataviz", "args": "bar chart" }), &Workspace::new("/"));
    assert!(!loaded.is_error);
    assert!(loaded.content.starts_with("Skill dataviz (base directory for its files: "));
    assert!(loaded.content.contains("Do the dataviz thing.") && loaded.content.ends_with("Arguments: bar chart"));
    assert!(!loaded.content.contains("description:"), "the frontmatter stays behind");
    assert!(tool.run(&json!({ "skill": "missing" }), &Workspace::new("/")).is_error);
}
