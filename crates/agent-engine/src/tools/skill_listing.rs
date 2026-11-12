use super::skill::FoundSkill;

const DESCRIPTION_CHARS: usize = 1536;
const LISTING_CHARS: usize = 8_000;
const SHORTEST: usize = 60;

fn shortened(text: &str, limit: usize) -> String {
    match text.char_indices().nth(limit) {
        Some((at, _)) => format!("{}…", text[..at].trim_end()),
        None => text.to_owned(),
    }
}

pub fn listing(skills: &[FoundSkill]) -> String {
    let offered: Vec<&FoundSkill> = skills.iter().filter(|skill| skill.model_invocation).collect();
    if offered.is_empty() {
        return String::new();
    }
    let names: usize = offered.iter().map(|skill| skill.name.chars().count() + 4).sum();
    let room = LISTING_CHARS.saturating_sub(names) / offered.len();
    let limit = room.min(DESCRIPTION_CHARS);
    offered
        .iter()
        .map(|skill| match limit {
            0..SHORTEST => format!("- {}", skill.name),
            _ => format!("- {}: {}", skill.name, shortened(&skill.description, limit)),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(name: &str, description: &str) -> FoundSkill {
        FoundSkill {
            name: name.into(),
            description: description.into(),
            file: Default::default(),
            model_invocation: true,
            trusted: true,
        }
    }

    #[test]
    fn a_long_description_is_cut() {
        let text = listing(&[skill("tdd", &"word ".repeat(1000))]);
        assert!(text.chars().count() < DESCRIPTION_CHARS + 20 && text.ends_with('…'));
    }

    #[test]
    fn many_skills_share_the_budget_and_finally_show_only_their_names() {
        let few: Vec<FoundSkill> = (0..20).map(|n| skill(&format!("s{n}"), &"d".repeat(1000))).collect();
        assert!(listing(&few).chars().count() <= LISTING_CHARS + 20 * 2);
        let many: Vec<FoundSkill> = (0..400).map(|n| skill(&format!("s{n}"), "does things")).collect();
        assert!(listing(&many).lines().all(|line| !line.contains(':')));
    }

    #[test]
    fn hidden_skills_stay_out() {
        let hidden = FoundSkill { model_invocation: false, ..skill("secret", "x") };
        assert_eq!(listing(&[hidden, skill("tdd", "Tests first.")]), "- tdd: Tests first.");
    }
}
