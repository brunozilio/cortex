//! The app menu's Settings item, kept in the interface language.

use superfik_menu::AppMenu;

pub struct Menu {
    menu: Box<dyn AppMenu>,
    title: String,
}

impl std::fmt::Debug for Menu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menu").field("title", &self.title).finish_non_exhaustive()
    }
}

impl Menu {
    pub fn new(menu: Box<dyn AppMenu>, title: String) -> Self {
        Self { menu, title }
    }

    pub fn sync(&mut self, title: String) {
        if title != self.title {
            self.menu.set_settings_title(&title);
            self.title = title;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    struct Shared(Rc<RefCell<Vec<String>>>);

    impl AppMenu for Shared {
        fn set_settings_title(&mut self, title: &str) {
            self.0.borrow_mut().push(title.to_owned());
        }
    }

    #[test]
    fn retitles_only_when_the_title_changes() {
        let titles = Rc::new(RefCell::new(Vec::new()));
        let mut menu = Menu::new(Box::new(Shared(titles.clone())), "Settings…".into());
        menu.sync("Settings…".into());
        menu.sync("Ajustes…".into());
        menu.sync("Ajustes…".into());
        assert_eq!(*titles.borrow(), vec!["Ajustes…"]);
    }
}
