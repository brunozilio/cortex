const LINES: usize = 5;
const CHARS: usize = 1_000;

#[derive(Debug, Default)]
pub struct Pastes {
    texts: Vec<String>,
}

impl Pastes {
    pub fn fold(&mut self, text: &str) -> Option<String> {
        let lines = text.lines().count();
        if lines < LINES && text.chars().count() < CHARS {
            return None;
        }
        self.texts.push(text.to_owned());
        Some(format!("[Pasted text #{} +{lines} lines]", self.texts.len()))
    }

    pub fn unfold(&self, text: &str) -> String {
        let mut out = text.to_owned();
        for (at, paste) in self.texts.iter().enumerate() {
            let number = at + 1;
            let lines = paste.lines().count();
            out = out.replace(&format!("[Pasted text #{number} +{lines} lines]"), paste);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_paste_folds_and_unfolds_where_it_was_typed() {
        let mut pastes = Pastes::default();
        assert_eq!(pastes.fold("one line"), None, "short pastes stay as they are");
        let log = "a\nb\nc\nd\ne\nf";
        let placeholder = pastes.fold(log).expect("folded");
        assert_eq!(placeholder, "[Pasted text #1 +6 lines]");
        let typed = format!("why does this fail?\n{placeholder}\nthanks");
        assert_eq!(pastes.unfold(&typed), format!("why does this fail?\n{log}\nthanks"));
        assert_eq!(pastes.fold(&"x".repeat(1_200)).as_deref(), Some("[Pasted text #2 +1 lines]"));
    }
}
