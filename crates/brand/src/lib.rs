include!(concat!(env!("OUT_DIR"), "/brand.rs"));

pub fn env(key: &str) -> String {
    format!("{ENV}_{key}")
}

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
