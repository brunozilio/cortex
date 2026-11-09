use super::*;
use crate::text::{FontWeight, TextStyle, TextSystem};

const BODY: TextStyle = TextStyle::new(14.0, 20.0, FontWeight::Regular);
const SAMPLE: &str = "日本語 中文 한국어";
const FIXTURE: &[u8] = include_bytes!("../../tests/fixtures/cjk-sample.otf");

fn notdefs(text: &mut TextSystem, sample: &str, style: TextStyle) -> usize {
    let layout = text.layout(sample, style, None);
    layout.lines.iter().flat_map(|l| &l.glyphs).filter(|g| g.glyph_id == 0).count()
}

#[derive(Default)]
struct Fixture(std::sync::Arc<std::sync::Mutex<Vec<Script>>>);

impl FallbackFaces for Fixture {
    fn load(&mut self, script: Script, db: &mut fontdb::Database) -> bool {
        if let Ok(mut asked) = self.0.lock() {
            asked.push(script);
        }
        db.load_font_data(FIXTURE.to_vec());
        true
    }
}

#[test]
fn scripts_are_told_apart() {
    assert_eq!(Script::of('a'), None);
    assert_eq!(Script::of('é'), None);
    assert_eq!(Script::of('日'), Some(Script::Han));
    assert_eq!(Script::of('か'), Some(Script::Han));
    assert_eq!(Script::of('。'), Some(Script::Han));
    assert_eq!(Script::of('한'), Some(Script::Hangul));
    let found: Vec<_> = Scripts::in_text(SAMPLE).iter().collect();
    assert_eq!(found, vec![Script::Han, Script::Hangul]);
    assert_eq!(Scripts::in_text("Ação — ok"), Scripts::default());
}

#[test]
fn without_a_fallback_cjk_is_boxes_and_reported_missing_once() {
    let mut text = TextSystem::without_system_fonts();
    assert!(notdefs(&mut text, SAMPLE, BODY) > 0);
    assert_eq!(text.take_missing(), vec![Script::Han, Script::Hangul]);
    notdefs(&mut text, "日本", BODY);
    assert!(text.take_missing().is_empty(), "each script is handed out once");
}

#[test]
fn a_face_added_later_replaces_the_boxes() {
    let mut text = TextSystem::without_system_fonts();
    assert!(notdefs(&mut text, SAMPLE, BODY) > 0);
    assert!(text.add_face(Script::Han, FIXTURE.to_vec()));
    assert_eq!(notdefs(&mut text, SAMPLE, BODY), 0, "the cached boxes are dropped");
    assert_eq!(notdefs(&mut text, SAMPLE, BODY.mono()), 0);
    assert!(!text.add_face(Script::Hangul, b"not a font".to_vec()));
}

#[test]
fn a_fallback_is_sought_only_for_text_that_needs_it_and_only_once() {
    let fixture = Fixture::default();
    let asked = fixture.0.clone();
    let mut text = TextSystem::with_fonts(None, Box::new(fixture));
    notdefs(&mut text, "Research — ação", BODY);
    assert!(asked.lock().map(|a| a.is_empty()).unwrap_or(false));
    assert_eq!(notdefs(&mut text, SAMPLE, BODY), 0);
    assert_eq!(notdefs(&mut text, SAMPLE, TextStyle::new(14.0, 20.0, FontWeight::SemiBold)), 0);
    assert_eq!(asked.lock().map(|a| a.clone()).unwrap_or_default(), vec![Script::Han, Script::Hangul]);
    assert!(text.take_missing().is_empty());
}

#[test]
fn latin_text_shapes_the_same_with_a_fallback_loaded() {
    let latin = "What are we working on? Ação, über, naïve.";
    let mut plain = TextSystem::without_system_fonts();
    let before = plain.layout(latin, BODY, Some(200.0));
    let mut mixed = TextSystem::with_fonts(None, Box::new(Fixture::default()));
    mixed.layout(SAMPLE, BODY, None);
    let after = mixed.layout(latin, BODY, Some(200.0));
    let glyphs = |l: &crate::text::TextLayout| -> Vec<(u16, u32)> {
        l.lines.iter().flat_map(|l| &l.glyphs).map(|g| (g.glyph_id, g.x.to_bits())).collect()
    };
    assert_eq!(glyphs(&before), glyphs(&after));
}

#[test]
fn this_systems_faces_draw_cjk() {
    let mut text = TextSystem::new();
    let boxes = notdefs(&mut text, SAMPLE, BODY);
    let missing = text.take_missing();
    assert!(boxes == 0 || !missing.is_empty(), "{boxes} boxes, none reported missing");
    if cfg!(target_os = "macos") {
        assert_eq!((boxes, missing), (0, vec![]));
    }
}

#[test]
fn only_faces_with_drawable_outlines_count() {
    assert!(has_outlines(FIXTURE, 0), "CFF");
    let mut hvgl = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
    hvgl.extend_from_slice(b"hvgl");
    hvgl.extend_from_slice(&[0; 12]);
    assert!(!has_outlines(&hvgl, 0));
    let mut collection = b"ttcf\0\x01\0\0\0\0\0\x01\0\0\0\x10".to_vec();
    collection.extend_from_slice(&hvgl);
    assert!(!has_outlines(&collection, 0));
    collection[16 + 12..16 + 16].copy_from_slice(b"glyf");
    assert!(has_outlines(&collection, 0));
    assert!(!has_outlines(&[], 0) && !has_outlines(b"ttcf", 3));
}
