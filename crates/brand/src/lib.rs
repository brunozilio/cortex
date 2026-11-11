//! The product's name, command, folders and owner. They are fixed when it
//! is built, from the workspace's `brand.toml`; nothing else spells them
//! out. Texts kept in files (prompts, skills) name them with placeholders
//! that [`text`] fills in.

include!(concat!(env!("OUT_DIR"), "/brand.rs"));

/// The environment variable `key` of this product (`{ENV}_<key>`).
pub fn env(key: &str) -> String {
    format!("{ENV}_{key}")
}

/// `template` with its placeholders filled in: `{{NAME}}`, `{{COMMAND}}`,
/// `{{DIR}}`, `{{INSTRUCTIONS}}`, `{{LOCAL_INSTRUCTIONS}}`, `{{PLUGIN_DIR}}`,
/// `{{ENV}}`, `{{COMPANY}}` and `{{REPOSITORY}}`.
pub fn text(template: &str) -> String {
    if !template.contains("{{") {
        return template.to_owned();
    }
    [
        ("{{NAME}}", NAME),
        ("{{COMMAND}}", COMMAND),
        ("{{DIR}}", DIR),
        ("{{INSTRUCTIONS}}", INSTRUCTIONS),
        ("{{LOCAL_INSTRUCTIONS}}", LOCAL_INSTRUCTIONS),
        ("{{PLUGIN_DIR}}", PLUGIN_DIR),
        ("{{ENV}}", ENV),
        ("{{COMPANY}}", COMPANY),
        ("{{REPOSITORY}}", REPOSITORY),
    ]
    .iter()
    .fold(template.to_owned(), |text, (placeholder, value)| text.replace(placeholder, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_names_derive_from_the_command() {
        assert_eq!(DIR, format!(".{COMMAND}"));
        assert_eq!(ENV, COMMAND.to_ascii_uppercase().replace('-', "_"));
        assert_eq!(INSTRUCTIONS, format!("{ENV}.md"));
        assert_eq!(env("PROJECT_DIR"), format!("{ENV}_PROJECT_DIR"));
    }

    #[test]
    fn placeholders_are_filled_and_other_braces_kept() {
        let filled = text("Run `{{COMMAND}} login`; {{NAME}} reads {{DIR}}/{{INSTRUCTIONS}}. {x}");
        assert_eq!(filled, format!("Run `{COMMAND} login`; {NAME} reads {DIR}/{INSTRUCTIONS}. {{x}}"));
        assert_eq!(text("plain"), "plain");
    }
}

#[cfg(test)]
mod literal_tests {
    #[test]
    fn each_name_is_also_a_literal() {
        const JOINED: &str = concat!(name!(), " ", command!(), " ", dir!(), " ", env_prefix!());
        assert_eq!(JOINED, format!("{} {} {} {}", super::NAME, super::COMMAND, super::DIR, super::ENV));
    }
}
