use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "en")]
    English,
    #[serde(rename = "pt-BR")]
    Portuguese,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::English, Language::Portuguese];

    pub const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Portuguese => "pt-BR",
        }
    }

    pub const fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Portuguese => "Português",
        }
    }

    pub fn from_locale(tag: &str) -> Option<Language> {
        let primary = tag.split(['-', '_', '.', '@']).next().unwrap_or_default().to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Language::English),
            "pt" => Some(Language::Portuguese),
            _ => None,
        }
    }

    pub fn first_offered<S: AsRef<str>>(locales: impl IntoIterator<Item = S>) -> Option<Language> {
        locales.into_iter().find_map(|tag| Language::from_locale(tag.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_match_on_their_primary_subtag() {
        assert_eq!(Language::from_locale("pt-BR"), Some(Language::Portuguese));
        assert_eq!(Language::from_locale("pt_PT.UTF-8"), Some(Language::Portuguese));
        assert_eq!(Language::from_locale("EN-us"), Some(Language::English));
        assert_eq!(Language::from_locale("fr-FR"), None);
        assert_eq!(Language::from_locale(""), None);
        assert_eq!(Language::first_offered(["fr", "pt-BR", "en"]), Some(Language::Portuguese));
        assert_eq!(Language::first_offered(["fr", "de"]), None);
    }

    #[test]
    fn serialises_by_tag() {
        assert_eq!(serde_json::to_string(&Language::Portuguese).expect("json"), r#""pt-BR""#);
        for language in Language::ALL {
            let json = serde_json::to_string(&language).expect("json");
            assert_eq!(json, format!("\"{}\"", language.code()));
            assert_eq!(serde_json::from_str::<Language>(&json).expect("parses"), language);
        }
    }
}
