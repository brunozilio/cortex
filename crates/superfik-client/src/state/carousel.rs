//! Long node lists shown a page at a time (Skills, Plugins): the page each
//! session shows, kept in range as rows come and go. Pure and tested.

use superfik_ui::theme::metrics;

use super::{Effect, Session};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Carousel {
    Skills,
    Plugins,
    Commands,
    Notes,
}

pub fn page_count(rows: usize) -> usize {
    rows.div_ceil(metrics::PAGE_ROWS).max(1)
}

impl Session {
    fn carousel_rows(&self, carousel: Carousel) -> usize {
        match carousel {
            Carousel::Skills => self.selected_skills().len(),
            Carousel::Plugins => self.plugin_rows().len(),
            Carousel::Commands => super::Command::ALL.len(),
            Carousel::Notes => 0,
        }
    }

    pub fn pages(&self, carousel: Carousel) -> usize {
        page_count(self.carousel_rows(carousel))
    }

    pub fn page(&self, carousel: Carousel) -> usize {
        let page = self.conversation().pages.get(&carousel).copied().unwrap_or_default();
        page.min(self.pages(carousel) - 1)
    }

    pub(super) fn flip(&mut self, carousel: Carousel, by: i32) -> Vec<Effect> {
        let last = self.pages(carousel) as i64 - 1;
        let page = (self.page(carousel) as i64 + i64::from(by)).clamp(0, last) as usize;
        self.conversation_mut().pages.insert(carousel, page);
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{Skill, SkillIcon};

    use super::*;
    use crate::state::Action;

    fn session(skills: usize) -> Session {
        let mut s = Session::new(Some("/repo".into()));
        s.available_skills = (0..skills)
            .map(|i| Skill {
                name: format!("skill-{i}"),
                title: String::new(),
                description: String::new(),
                icon: SkillIcon::Cube,
            })
            .collect();
        s
    }

    #[test]
    fn pages_stop_at_either_end() {
        let mut s = session(12);
        assert_eq!(s.pages(Carousel::Skills), 3);
        s.apply(Action::Page(Carousel::Skills, -1), 0.0);
        assert_eq!(s.page(Carousel::Skills), 0);
        s.apply(Action::Page(Carousel::Skills, 1), 0.0);
        s.apply(Action::Page(Carousel::Skills, 5), 0.0);
        assert_eq!(s.page(Carousel::Skills), 2);
        assert_eq!(session(0).pages(Carousel::Skills), 1, "an empty node is one page");
        assert_eq!(s.page(Carousel::Plugins), 0, "each carousel keeps its own page");
    }

    #[test]
    fn removing_the_last_row_of_the_last_page_steps_back() {
        let mut s = session(11);
        s.apply(Action::Page(Carousel::Skills, 2), 0.0);
        s.apply(Action::RemoveSkill("skill-10".into()), 0.0);
        assert_eq!(s.page(Carousel::Skills), 1);
        assert_eq!(s.conversation().pages.get(&Carousel::Skills), Some(&1));
    }

    #[test]
    fn each_session_keeps_its_own_page() {
        let mut s = session(12);
        s.apply(Action::Page(Carousel::Skills, 1), 0.0);
        s.apply(Action::NewConversation, 0.0);
        assert_eq!(s.page(Carousel::Skills), 0);
    }
}
