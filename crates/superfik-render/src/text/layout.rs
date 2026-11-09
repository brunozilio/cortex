//! Laid-out text: lines of positioned glyphs in logical pixels.

use cosmic_text::LayoutGlyph;

use super::system::{FontFamily, FontWeight};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: FontWeight,
    pub tracking: f32,
    pub family: FontFamily,
}

impl TextStyle {
    pub const fn new(size: f32, line_height: f32, weight: FontWeight) -> Self {
        Self { size, line_height, weight, tracking: 0.0, family: FontFamily::Sans }
    }

    pub const fn mono(self) -> Self {
        Self { family: FontFamily::Mono, ..self }
    }

    pub const fn tracked(self, tracking: f32) -> Self {
        Self { tracking, ..self }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleRun {
    pub start: usize,
    pub end: usize,
    pub weight: Option<FontWeight>,
    pub family: Option<FontFamily>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flow {
    pub width: f32,
    pub inset_after: usize,
    pub inset: f32,
}

impl Flow {
    pub const fn plain(width: f32) -> Self {
        Self { width, inset_after: usize::MAX, inset: 0.0 }
    }
}

#[derive(Debug, Clone)]
pub struct TextLine {
    pub top: f32,
    pub baseline: f32,
    pub width: f32,
    pub start: usize,
    pub glyphs: Vec<LayoutGlyph>,
}

#[derive(Debug, Clone)]
pub struct TextLayout {
    pub text: String,
    pub width: f32,
    pub height: f32,
    pub lines: Vec<TextLine>,
}

impl TextLayout {
    pub fn word_index(&self, offset: usize) -> usize {
        let prefix = &self.text[..offset.min(self.text.len())];
        prefix.split_whitespace().count().saturating_sub(usize::from(!prefix.ends_with(char::is_whitespace)))
    }

    pub fn caret_x(&self, offset: usize) -> f32 {
        let mut x = 0.0;
        for line in &self.lines {
            for glyph in &line.glyphs {
                if glyph.end <= offset {
                    x = glyph.x + glyph.w;
                }
            }
        }
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(text: &str) -> TextLayout {
        TextLayout { text: text.into(), width: 0.0, height: 0.0, lines: vec![] }
    }

    #[test]
    fn word_index_counts_preceding_words() {
        let l = layout("deep blue overhead");
        assert_eq!(l.word_index(0), 0);
        assert_eq!(l.word_index(3), 0);
        assert_eq!(l.word_index(5), 1);
        assert_eq!(l.word_index(11), 2);
    }
}
