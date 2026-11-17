use super::app::App;

pub fn agent(app: &mut App, what: &str) {
    send(
        app,
        format!(
            "Create a {brand_name} sub-agent for this: {what}\n\nWrite it to {brand_dir}/agents/<name>.md: YAML \
             frontmatter with `name` (lowercase, hyphens), `description` (when to use it, one or two sentences), \
             and `tools` only if it should have fewer than all; then its instructions as the body. Ask me first \
             if what it should do is unclear.",
            brand_name = brand::NAME,
            brand_dir = brand::DIR
        ),
    );
}

pub fn skill(app: &mut App, what: &str) {
    send(
        app,
        format!(
            "Create a {brand_name} skill for this: {what}\n\nLoad the skill-creator skill and follow it. Write the \
             skill under {brand_dir}/skills/<name>/SKILL.md.",
            brand_name = brand::NAME,
            brand_dir = brand::DIR
        ),
    );
}

fn send(app: &mut App, prompt: String) {
    if app.busy() {
        return app.transcript.notice("Wait for the agent to finish first.");
    }
    app.send(prompt);
}
