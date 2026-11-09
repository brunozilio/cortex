//! Font loading, shaping and a layout cache.

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, Weight, Wrap, fontdb};

use super::fallback::{FallbackFaces, NoFaces, Script, Scripts, SystemFaces};
use super::layout::{Flow, StyleRun, TextLayout, TextLine, TextStyle};

pub(crate) const FAMILY: &str = "Inter";
pub(crate) const FONTS: [&[u8]; 3] = [
    include_bytes!("../../../../assets/fonts/Inter-Regular.otf"),
    include_bytes!("../../../../assets/fonts/Inter-Medium.otf"),
    include_bytes!("../../../../assets/fonts/Inter-SemiBold.otf"),
];
const CACHE_LIMIT: usize = 4096;
pub(crate) const BUNDLED_MONO: &[u8] = include_bytes!("../../../../assets/fonts/FiraMono-Medium.ttf");
const SYSTEM_MONO: [&str; 5] = [
    "/System/Library/Fonts/SFNSMono.ttf",
    "/System/Library/Fonts/Menlo.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FontFamily {
    #[default]
    Sans,
    Mono,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    Medium,
    SemiBold,
}

impl FontWeight {
    fn to_cosmic(self) -> Weight {
        match self {
            Self::Regular => Weight::NORMAL,
            Self::Medium => Weight::MEDIUM,
            Self::SemiBold => Weight::SEMIBOLD,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LayoutKey {
    text: String,
    family: FontFamily,
    size: u32,
    line_height: u32,
    weight: FontWeight,
    tracking: u32,
    max_width: Option<u32>,
    inset: Option<(usize, u32)>,
    spaced: Vec<(usize, u32)>,
    runs: Vec<StyleRun>,
}

pub struct TextSystem {
    fonts: FontSystem,
    cache: HashMap<LayoutKey, Arc<TextLayout>>,
    fallback: Box<dyn FallbackFaces>,
    sought: Scripts,
    missing: Scripts,
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem").field("cached", &self.cache.len()).finish()
    }
}

impl Default for TextSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSystem {
    pub fn new() -> Self {
        let windows = super::windows_fonts_dir().map(|root| root.join("consola.ttf"));
        let mono = SYSTEM_MONO.iter().map(std::path::Path::new).chain(windows.as_deref()).find(|path| path.is_file());
        Self::with_fonts(mono, Box::new(SystemFaces))
    }

    pub fn without_system_fonts() -> Self {
        Self::with_fonts(None, Box::new(NoFaces))
    }

    pub fn with_fonts(system_mono: Option<&std::path::Path>, fallback: Box<dyn FallbackFaces>) -> Self {
        let mut db = fontdb::Database::new();
        for font in FONTS {
            db.load_font_data(font.to_vec());
        }
        let loaded = system_mono.is_some_and(|path| match db.load_font_file(path) {
            Ok(()) => true,
            Err(error) => {
                tracing::debug!(%error, path = %path.display(), "monospace font not loaded");
                false
            }
        });
        if !loaded {
            db.load_font_data(BUNDLED_MONO.to_vec());
        }
        if let Some(family) = db.faces().last().and_then(|face| face.families.first()).map(|(name, _)| name.clone()) {
            db.set_monospace_family(family);
        }
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: HashMap::new(),
            fallback,
            sought: Scripts::default(),
            missing: Scripts::default(),
        }
    }

    pub fn fonts_mut(&mut self) -> &mut FontSystem {
        &mut self.fonts
    }

    pub fn take_missing(&mut self) -> Vec<Script> {
        std::mem::take(&mut self.missing).iter().collect()
    }

    pub fn add_face(&mut self, script: Script, data: Vec<u8>) -> bool {
        let before = self.fonts.db().len();
        self.fonts.db_mut().load_font_data(data);
        let added = self.fonts.db().len() > before;
        if added {
            self.sought.insert(script);
            self.cache.clear();
        }
        added
    }

    fn seek_faces(&mut self, text: &str) {
        let sought = self.sought;
        for script in Scripts::in_text(text).iter().filter(|&s| !sought.contains(s)) {
            self.sought.insert(script);
            if !self.fallback.load(script, self.fonts.db_mut()) {
                self.missing.insert(script);
            }
        }
    }

    pub fn layout(&mut self, text: &str, style: TextStyle, max_width: Option<f32>) -> Arc<TextLayout> {
        match max_width {
            Some(width) => self.layout_flow(text, style, Flow::plain(width)),
            None => self.layout_keyed(text, style, None, None, (&[], &[])),
        }
    }

    pub fn layout_flow(&mut self, text: &str, style: TextStyle, flow: Flow) -> Arc<TextLayout> {
        self.layout_spaced(text, style, flow, &[])
    }

    pub fn layout_spaced(
        &mut self,
        text: &str,
        style: TextStyle,
        flow: Flow,
        spaced: &[(usize, f32)],
    ) -> Arc<TextLayout> {
        self.layout_styled(text, style, flow, spaced, &[])
    }

    pub fn layout_styled(
        &mut self,
        text: &str,
        style: TextStyle,
        flow: Flow,
        spaced: &[(usize, f32)],
        runs: &[StyleRun],
    ) -> Arc<TextLayout> {
        let inset = (flow.inset > 0.0 && flow.inset_after != usize::MAX).then_some((flow.inset_after, flow.inset));
        self.layout_keyed(text, style, Some(flow.width), inset, (spaced, runs))
    }

    fn layout_keyed(
        &mut self,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        inset: Option<(usize, f32)>,
        (spaced, runs): (&[(usize, f32)], &[StyleRun]),
    ) -> Arc<TextLayout> {
        let key = LayoutKey {
            text: text.to_owned(),
            family: style.family,
            size: style.size.to_bits(),
            line_height: style.line_height.to_bits(),
            weight: style.weight,
            tracking: style.tracking.to_bits(),
            max_width: max_width.map(|w| w.max(1.0).floor() as u32),
            inset: inset.map(|(after, by)| (after, by.to_bits())),
            spaced: spaced.iter().map(|&(at, em)| (at, em.to_bits())).collect(),
            runs: runs.to_vec(),
        };
        if let Some(layout) = self.cache.get(&key) {
            return layout.clone();
        }
        if self.cache.len() > CACHE_LIMIT {
            self.cache.clear();
        }
        self.seek_faces(text);
        let width = key.max_width.map(|w| w as f32);
        let layout = match inset {
            Some((after, by)) => self.shape_inset(text, style, width.unwrap_or(f32::MAX), (after, by), (spaced, runs)),
            None => self.shape(text, style, width, (spaced, runs)),
        };
        let layout = Arc::new(layout);
        self.cache.insert(key, layout.clone());
        layout
    }

    pub fn measure(&mut self, text: &str, style: TextStyle, max_width: Option<f32>) -> (f32, f32) {
        let layout = self.layout(text, style, max_width);
        (layout.width, layout.height)
    }

    fn shape_inset(
        &mut self,
        text: &str,
        style: TextStyle,
        width: f32,
        (after, inset): (usize, f32),
        (spaced, runs): (&[(usize, f32)], &[StyleRun]),
    ) -> TextLayout {
        let head = self.shape(text, style, Some(width), (spaced, runs));
        let Some(start) = head.lines.get(after).map(|l| l.start) else {
            return head;
        };
        let rebased: Vec<_> = spaced.iter().filter(|(at, _)| *at >= start).map(|&(at, em)| (at - start, em)).collect();
        let rerun: Vec<_> = runs
            .iter()
            .filter(|r| r.end > start)
            .map(|r| StyleRun { start: r.start.saturating_sub(start), end: r.end - start, ..*r })
            .collect();
        let tail = self.shape(&text[start..], style, Some((width - inset).max(1.0)), (&rebased, &rerun));
        let top = after as f32 * style.line_height;
        let mut lines: Vec<TextLine> = head.lines.into_iter().take(after).collect();
        let mut layout_width = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        for mut line in tail.lines {
            line.start += start;
            for glyph in &mut line.glyphs {
                glyph.start += start;
                glyph.end += start;
                glyph.x += inset;
            }
            line.top += top;
            line.baseline += top;
            line.width += inset;
            layout_width = layout_width.max(line.width);
            lines.push(line);
        }
        let height = lines.last().map_or(style.line_height, |l| l.top + style.line_height);
        TextLayout { text: text.to_owned(), width: layout_width.ceil(), height, lines }
    }

    fn shape(
        &mut self,
        text: &str,
        style: TextStyle,
        max_width: Option<f32>,
        (spaced, runs): (&[(usize, f32)], &[StyleRun]),
    ) -> TextLayout {
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(style.size, style.line_height));
        let mut buffer = buffer.borrow_with(&mut self.fonts);
        buffer.set_wrap(if max_width.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
        buffer.set_size(max_width, None);
        let attrs_for = |run: Option<&StyleRun>| {
            let family = match run.and_then(|r| r.family).unwrap_or(style.family) {
                FontFamily::Sans => Family::Name(FAMILY),
                FontFamily::Mono => Family::Monospace,
            };
            Attrs::new().family(family).weight(run.and_then(|r| r.weight).unwrap_or(style.weight).to_cosmic())
        };
        let attrs = attrs_for(None);
        if spaced.is_empty() && runs.is_empty() {
            let attrs = if style.tracking == 0.0 { attrs } else { attrs.letter_spacing(style.tracking) };
            buffer.set_text(text, &attrs, Shaping::Advanced, None);
        } else {
            buffer.set_rich_text(
                super::spans::spans(text, style.tracking, spaced, runs, attrs_for),
                &attrs,
                Shaping::Advanced,
                None,
            );
        }
        buffer.shape_until_scroll(false);

        let mut paragraphs = Vec::with_capacity(buffer.lines.len());
        let mut at = 0;
        for line in &buffer.lines {
            paragraphs.push(at);
            at += line.text().len() + line.ending().as_str().len();
        }
        let mut lines = Vec::new();
        let (mut width, mut height) = (0.0_f32, 0.0_f32);
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
            let base = paragraphs.get(run.line_i).copied().unwrap_or(0);
            let mut glyphs = run.glyphs.to_vec();
            for glyph in &mut glyphs {
                glyph.start += base;
                glyph.end += base;
            }
            let start = glyphs.first().map_or(base, |g| g.start);
            lines.push(TextLine { top: run.line_top, baseline: run.line_y, width: run.line_w, start, glyphs });
        }
        if text.ends_with('\n') && lines.last().is_none_or(|l| l.start < text.len() && !l.glyphs.is_empty()) {
            let top = lines.last().map_or(0.0, |l| l.top + style.line_height);
            let baseline = lines.last().map_or(0.0, |l| l.baseline + style.line_height);
            lines.push(TextLine { top, baseline, width: 0.0, start: text.len(), glyphs: Vec::new() });
            height = height.max(top + style.line_height);
        }
        if lines.is_empty() {
            height = style.line_height;
        }
        TextLayout { text: text.to_owned(), width: width.ceil(), height, lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: TextStyle = TextStyle::new(14.0, 20.0, FontWeight::Regular);

    #[test]
    fn without_a_system_mono_the_bundled_one_is_monospaced() {
        let mut text = TextSystem::without_system_fonts();
        let (narrow, _) = text.measure("iiii", BODY.mono(), None);
        let (wide, _) = text.measure("MMMM", BODY.mono(), None);
        assert!((narrow - wide).abs() < 0.5, "iiii {narrow} vs MMMM {wide}");
        let (sans_narrow, _) = text.measure("iiii", BODY, None);
        assert!(sans_narrow < narrow, "Inter stays proportional");
    }

    #[test]
    fn measures_single_line() {
        let mut text = TextSystem::new();
        let (w, h) = text.measure("Research", BODY, None);
        assert!(w > 40.0 && w < 80.0, "width {w}");
        assert_eq!(h, 20.0);
    }

    #[test]
    fn wraps_at_max_width() {
        let mut text = TextSystem::new();
        let layout = text.layout("What are we working on? I can tune how something moves.", BODY, Some(120.0));
        assert!(layout.lines.len() > 1);
        assert!(layout.width <= 120.0);
    }

    #[test]
    fn tracking_tightens_text() {
        let mut text = TextSystem::new();
        let (normal, _) = text.measure("Research", BODY, None);
        let (tight, _) = text.measure("Research", BODY.tracked(-0.05), None);
        assert!(tight < normal - 3.0, "{tight} vs {normal}");
    }

    #[test]
    fn a_bold_run_is_wider_and_keeps_the_bytes() {
        let mut text = TextSystem::new();
        let flow = Flow::plain(400.0);
        let plain = text.layout_flow("make it heavy now", BODY, flow);
        let run = StyleRun { start: 8, end: 13, weight: Some(FontWeight::SemiBold), family: None };
        let bold = text.layout_styled("make it heavy now", BODY, flow, &[], &[run]);
        assert!(bold.width > plain.width, "{} vs {}", bold.width, plain.width);
        assert_eq!(bold.lines[0].glyphs.last().map(|g| g.end), Some(17));
    }

    #[test]
    fn layouts_are_cached() {
        let mut text = TextSystem::new();
        let a = text.layout("hey", BODY, None);
        let b = text.layout("hey", BODY, None);
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn lines_after_the_inset_point_are_narrower_and_keep_their_offsets() {
        let mut text = TextSystem::new();
        let sample = "deep blue overhead and a faint glow at the horizon where the moon sits as a soft disc";
        let plain = text.layout(sample, BODY, Some(160.0));
        let flow = text.layout_flow(sample, BODY, Flow { width: 160.0, inset_after: 2, inset: 60.0 });
        assert!(flow.lines.len() > plain.lines.len(), "{} vs {}", flow.lines.len(), plain.lines.len());
        for (a, b) in plain.lines.iter().zip(&flow.lines).take(2) {
            assert_eq!(a.glyphs.len(), b.glyphs.len());
            assert_eq!(a.glyphs[0].start, b.glyphs[0].start);
        }
        for (i, line) in flow.lines.iter().enumerate().skip(2) {
            assert_eq!(line.top, i as f32 * 20.0);
            assert!(line.glyphs[0].x >= 60.0, "line {i} starts at {}", line.glyphs[0].x);
            assert!(line.width <= 160.0);
        }
        let last = flow.lines.last().and_then(|l| l.glyphs.last()).map(|g| g.end);
        assert_eq!(last, Some(sample.len()));
        assert_eq!(flow.height, flow.lines.len() as f32 * 20.0);
    }

    #[test]
    fn a_flow_that_never_insets_matches_plain_wrapping() {
        let mut text = TextSystem::new();
        let plain = text.layout("What are we working on? I can tune", BODY, Some(120.0));
        let flow = text.layout_flow("What are we working on? I can tune", BODY, Flow::plain(120.0));
        assert!(Arc::ptr_eq(&plain, &flow));
    }

    #[test]
    fn offsets_count_from_the_whole_text_across_newlines() {
        let mut text = TextSystem::new();
        let layout = text.layout("ab\ncd\n", BODY, Some(200.0));
        let starts: Vec<Vec<usize>> = layout.lines.iter().map(|l| l.glyphs.iter().map(|g| g.start).collect()).collect();
        assert_eq!(starts, vec![vec![0, 1], vec![3, 4], vec![]]);
        assert_eq!(layout.lines.iter().map(|l| l.start).collect::<Vec<_>>(), vec![0, 3, 6]);
        assert_eq!(layout.height, 60.0);
    }

    #[test]
    fn empty_text_has_one_line_of_height() {
        let mut text = TextSystem::new();
        assert_eq!(text.measure("", BODY, None).1, 20.0);
    }
}
