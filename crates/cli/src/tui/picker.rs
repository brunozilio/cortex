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
}
