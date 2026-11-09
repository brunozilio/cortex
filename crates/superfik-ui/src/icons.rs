//! Icon set: Hugeicons (MIT, `icons/LICENSE-hugeicons.md`), stroke-rounded on
//! a 24×24 grid and recoloured white so they can be tinted. The model mark
//! (`BURST`), Get Started's cast (`COFFEE`, `PERSON`, `LOOP`) and the
//! browser card's `ARROW_RIGHT`, `RELOAD`, `INCOGNITO`, `IMPORT`,
//! `DELETE` and `KEY`, and the Notes node's `NOTE`, drawn in the same
//! stroke, are our own.

use superfik_render::Icon;

macro_rules! icon {
    ($name:ident, $file:literal) => {
        pub const $name: Icon = Icon::new(stringify!($name), include_str!(concat!("icons/", $file, ".svg")));
    };
}

icon!(REPOSITORY, "repository");
icon!(PULL_REQUEST, "git-pull-request");
icon!(BRANCH, "git-branch");
icon!(THUMB_UP, "thumbs-up");
icon!(QUESTION, "help-circle");
icon!(PLUS, "add-01");
icon!(SHIELD, "shield-01");
icon!(CARET_DOWN, "arrow-down-01");
icon!(CARET_UP, "arrow-up-01");
icon!(ARCHIVE, "archive-02");
icon!(PENCIL, "pencil-edit-02");
icon!(CHEVRON_LEFT, "arrow-left-01");
icon!(CHEVRON_RIGHT, "arrow-right-01");
icon!(CHEVRON_DOWN, "arrow-down-01");
icon!(PLUS_CIRCLE, "add-circle");
icon!(CLOSE_CIRCLE, "cancel-circle");
icon!(CLOSE, "cancel-01");
icon!(CHECK_CIRCLE, "checkmark-circle-02");
icon!(CHECK, "tick-02");
icon!(LAYERS, "layers-01");
icon!(IMAGE, "image-01");
icon!(GLOBE, "globe-02");
icon!(GLOBE_FILLED, "globe-02");
icon!(MIC, "mic-01");
icon!(ROBOT, "robot-02");
icon!(ARROW_UP, "arrow-up-02");
icon!(ARROW_LEFT, "arrow-left-02");
icon!(ARROW_RIGHT, "arrow-right-02");
icon!(RELOAD, "reload");
icon!(INCOGNITO, "incognito");
icon!(IMPORT, "import");
icon!(DELETE, "delete");
icon!(KEY, "key");
icon!(NOTE, "note");
icon!(STOP, "stop");
icon!(BOOKMARK, "bookmark-02");
icon!(DOCUMENT, "file-02");
icon!(CHAT, "bubble-chat");
icon!(PLAY, "play-circle");
icon!(BOLT, "flash");
icon!(SPARKLE, "sparkles");
icon!(BOOK, "book-open-01");
icon!(MORE, "more-horizontal");
icon!(MAP, "maps");
icon!(SEARCH, "search-01");
icon!(FLASK, "flask-conical");
icon!(GAUGE, "dashboard-speed-01");
icon!(CUBE, "cube");
icon!(BRAIN, "brain");
icon!(GEAR, "settings-01");
icon!(SPEAKER, "volume-high");
icon!(PLUG, "plug-01");
icon!(COFFEE, "coffee");
icon!(PERSON, "person");
icon!(LOOP, "loop");

pub const BURST: Icon = Icon::new(
    "BURST",
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" stroke="#fff" stroke-width="1.8" stroke-linecap="round"><circle cx="12" cy="12" r="3.6" fill="#fff" stroke="none"/><path d="M12 12L22.2 14.7M12 12L19.5 19.5M12 12L14.7 22.2M12 12L9.3 22.2M12 12L4.5 19.5M12 12L1.8 14.7M12 12L1.8 9.3M12 12L4.5 4.5M12 12L9.3 1.8M12 12L14.7 1.8M12 12L19.5 4.5M12 12L22.2 9.3"/></svg>"##,
);

pub const ALL: [Icon; 55] = [
    COFFEE,
    PERSON,
    LOOP,
    CHECK,
    SPEAKER,
    ARROW_LEFT,
    ARROW_RIGHT,
    RELOAD,
    INCOGNITO,
    IMPORT,
    DELETE,
    KEY,
    NOTE,
    SHIELD,
    THUMB_UP,
    QUESTION,
    PLUS,
    GEAR,
    PULL_REQUEST,
    GLOBE_FILLED,
    REPOSITORY,
    BRANCH,
    CARET_DOWN,
    CARET_UP,
    PENCIL,
    CHEVRON_DOWN,
    CHEVRON_LEFT,
    CHEVRON_RIGHT,
    PLUS_CIRCLE,
    CLOSE_CIRCLE,
    CLOSE,
    CHECK_CIRCLE,
    LAYERS,
    IMAGE,
    GLOBE,
    MIC,
    ROBOT,
    ARROW_UP,
    STOP,
    BOOKMARK,
    DOCUMENT,
    CHAT,
    PLAY,
    BOLT,
    SPARKLE,
    BOOK,
    MORE,
    MAP,
    SEARCH,
    FLASK,
    CUBE,
    GAUGE,
    BRAIN,
    BURST,
    PLUG,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_rasterizes_with_visible_pixels() {
        for icon in ALL {
            let pixels =
                superfik_render::icons::rasterize(&icon, 24, 24).unwrap_or_else(|| panic!("{} failed", icon.name));
            assert!(pixels.chunks(4).any(|p| p[3] > 0), "{} is empty", icon.name);
        }
    }

    #[test]
    fn icon_names_are_unique() {
        let mut names: Vec<&str> = ALL.iter().map(|i| i.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }
}
