use super::settings::Style;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Builtin {
    pub id: String,
    pub name: String,
    pub description: String,
    instructions: String,
    reminder: Option<String>,
}

impl Builtin {
    pub fn style(&self) -> Style {
        Style {
            name: self.name.clone(),
            instructions: self.instructions.clone(),
            keep_coding: true,
            reminder: self.reminder.clone(),
        }
    }
}

fn read(id: &str) -> Option<Builtin> {
    let text = content::text(&format!("styles/{id}.md"));
    let (front, body) = crate::frontmatter::parse(&text);
    Some(Builtin {
        id: id.to_owned(),
        name: front.get("name")?.to_owned(),
        description: front.get("description").unwrap_or_default().to_owned(),
        instructions: body.trim().to_owned(),
        reminder: front.get("turn-reminder").map(str::to_owned),
    })
}

pub fn all() -> Vec<Builtin> {
    content::builtin_files("styles/")
        .into_iter()
        .filter_map(|(path, _)| path.strip_prefix("styles/")?.strip_suffix(".md").and_then(read))
        .collect()
}

pub fn builtin(name: &str) -> Option<Builtin> {
    all().into_iter().find(|style| style.id.eq_ignore_ascii_case(name))
}

pub fn picked(requested: Option<&str>, from_settings: Option<Style>) -> Option<Style> {
    match requested {
        Some(name) if name.eq_ignore_ascii_case("default") => None,
        Some(name) => builtin(name).map(|style| style.style()).or(from_settings),
        None => from_settings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_s_style_wins_over_the_settings() {
        let settings = builtin("learning").map(|style| style.style());
        let name = |style: Option<Style>| style.map(|style| style.name);
        assert_eq!(name(picked(Some("concise"), settings.clone())).as_deref(), Some("Concise"));
        assert_eq!(name(picked(Some("default"), settings.clone())), None);
        assert_eq!(name(picked(None, settings.clone())).as_deref(), Some("Learning"));
        assert_eq!(name(picked(Some("unknown"), settings)).as_deref(), Some("Learning"));
    }

    #[test]
    fn built_in_styles_are_found_by_id_whatever_the_case() {
        assert_eq!(builtin("CONCISE").map(|style| style.name).as_deref(), Some("Concise"));
        assert_eq!(builtin("default"), None);
        let concise = builtin("concise").expect("concise").style();
        assert!(concise.keep_coding);
        assert!(concise.reminder.as_deref().is_some_and(|text| text.starts_with("Be concise")));
        assert!(builtin("learning").expect("learning").style().instructions.contains("TODO(human)"));
        assert_eq!(all().len(), 4);
        for style in all() {
            assert!(!style.style().instructions.contains("Claude"), "{} names another product", style.name);
        }
    }
}
