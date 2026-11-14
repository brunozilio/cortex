use crate::features::{
    Features, WEB_CLEANUP, WEB_EMPTY_PAGE_ERROR, WEB_FORMATS, WEB_LINKS, WEB_MAIN_CONTENT, WEB_SMALL_PAGES_DIRECT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebReading {
    pub main_content: bool,
    pub cleanup: bool,
    pub links: bool,
    pub small_pages_direct: bool,
    pub formats: bool,
    pub empty_page_error: bool,
}

impl Default for WebReading {
    fn default() -> Self {
        Self::from(&Features::default())
    }
}

impl From<&Features> for WebReading {
    fn from(features: &Features) -> Self {
        Self {
            main_content: features.on(WEB_MAIN_CONTENT),
            cleanup: features.on(WEB_CLEANUP),
            links: features.on(WEB_LINKS),
            small_pages_direct: features.on(WEB_SMALL_PAGES_DIRECT),
            formats: features.on(WEB_FORMATS),
            empty_page_error: features.on(WEB_EMPTY_PAGE_ERROR),
        }
    }
}

impl WebReading {
    pub fn none() -> Self {
        Self {
            main_content: false,
            cleanup: false,
            links: false,
            small_pages_direct: false,
            formats: false,
            empty_page_error: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_way_of_reading_a_page_follows_its_own_flag() {
        assert_eq!(
            WebReading::default(),
            WebReading {
                main_content: true,
                cleanup: true,
                links: true,
                small_pages_direct: true,
                formats: true,
                empty_page_error: true
            }
        );
        let features = Features::default()
            .layered(&json!({ WEB_MAIN_CONTENT: false, WEB_SMALL_PAGES_DIRECT: false }), true)
            .expect("flags");
        let reading = WebReading::from(&features);
        assert!(!reading.main_content && !reading.small_pages_direct);
        assert!(reading.cleanup && reading.links && reading.formats && reading.empty_page_error);
    }

    #[test]
    fn the_web_flags_are_experimental_and_listed() {
        let listed = Features::default().listed();
        for name in
            [WEB_MAIN_CONTENT, WEB_CLEANUP, WEB_LINKS, WEB_SMALL_PAGES_DIRECT, WEB_FORMATS, WEB_EMPTY_PAGE_ERROR]
        {
            let flag = listed.iter().find(|flag| flag.name == name);
            assert!(flag.is_some_and(|flag| flag.experimental && flag.on), "{name}");
        }
    }
}
