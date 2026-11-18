#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerKind {
    Model,
    Mode,
    Login,
    Rewind,
    Resume,
    Logout,
    OutputStyle,
    Config,
    Effort,
    Role(domain::model_roles::ModelRole),
    Account { removing: bool },
    PortBusy,
}

#[derive(Debug)]
pub struct Picker {
    pub kind: PickerKind,
    pub title: String,
    pub items: Vec<(String, String)>,
    pub cursor: usize,
}

impl Picker {
    pub fn up(&mut self) {
        self.cursor = self.cursor.checked_sub(1).unwrap_or(self.items.len().saturating_sub(1));
    }

    pub fn down(&mut self) {
        self.cursor = if self.cursor + 1 >= self.items.len() { 0 } else { self.cursor + 1 };
    }

    pub fn window(&self) -> std::ops::Range<usize> {
        super::commands::shown(self.items.len(), self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picker(len: usize, cursor: usize) -> Picker {
        let items = (0..len).map(|i| (format!("m{i}"), String::new())).collect();
        Picker { kind: PickerKind::Model, title: "Model".into(), items, cursor }
    }

    #[test]
    fn a_long_list_shows_a_window_that_follows_the_cursor() {
        assert_eq!(picker(30, 0).window(), 0..super::super::commands::SHOWN);
        assert_eq!(picker(30, 20).window(), 13..21);
        assert_eq!(picker(30, 29).window(), 22..30);
        assert_eq!(picker(3, 2).window(), 0..3);
    }

    #[test]
    fn wrapping_up_from_the_top_shows_the_last_items() {
        let mut picker = picker(30, 0);
        picker.up();
        assert_eq!(picker.window(), 22..30);
    }
}
