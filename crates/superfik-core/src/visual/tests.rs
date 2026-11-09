use super::*;

const NODE: &str = r#"<svg width="100%" viewBox="0 0 440 120">
<g class="node c-blue" onclick="sendPrompt('Tell me more about T-cells')">
  <rect x="100" y="20" width="180" height="44" rx="8"/>
  <text class="th" x="190" y="42" text-anchor="middle">T-cells</text>
</g>
<g transform="translate(10, 5)">
  <g class="node" onclick="sendPrompt(&quot;It\'s \&quot;hot\&quot;&quot;)"><circle cx="20" cy="30" r="5"/></g>
</g>
<g transform="rotate(30)"><rect onclick="sendPrompt('skip')" width="4" height="4"/></g>
</svg>"#;

#[test]
fn the_frame_comes_from_the_view_box_even_while_streaming() {
    let found = frame(r#"<svg width="100%" viewBox="0 0 440 260"><rect"#).expect("frame");
    assert_eq!(found, Frame { x: 0.0, y: 0.0, width: 440.0, height: 260.0 });
    assert_eq!(frame(r#"<svg width="300" height="150">"#).map(|f| f.ratio()), Some(0.5));
    assert_eq!(frame(r#"<svg viewBox="0 0 440"#), None, "the tag has not closed yet");
    assert_eq!(frame("<svg>"), None);
}

#[test]
fn send_prompt_calls_are_read_with_their_escapes() {
    assert_eq!(sent_prompt("sendPrompt('Hi there')").as_deref(), Some("Hi there"));
    assert_eq!(sent_prompt(r#"sendPrompt( "It's \"x\"" )"#).as_deref(), Some(r#"It's "x""#));
    assert_eq!(sent_prompt("sendPrompt(`a\\nb`)").as_deref(), Some("a\nb"));
    assert_eq!(sent_prompt("sendPrompt(name)"), None);
    assert_eq!(sent_prompt("sendPrompt('unterminated"), None);
    assert_eq!(sent_prompt("openLink('x')"), None);
}

#[test]
fn clickable_parts_cover_what_they_draw() {
    let prompts = prompts(NODE);
    assert_eq!(prompts.len(), 2, "the rotated one is left out: {prompts:?}");
    assert_eq!(prompts[0].text, "Tell me more about T-cells");
    assert_eq!(prompts[0].area, Area { x: 100.0, y: 20.0, w: 180.0, h: 44.0 });
    assert_eq!(prompts[1].text, r#"It's "hot""#);
    assert_eq!(prompts[1].area, Area { x: 25.0, y: 30.0, w: 10.0, h: 10.0 });
}

#[test]
fn areas_are_measured_from_the_frames_corner() {
    let svg =
        r#"<svg viewBox="50 10 440 100"><rect onclick="sendPrompt('a')" x="60" y="20" width="10" height="10"/></svg>"#;
    assert_eq!(prompts(svg)[0].area, Area { x: 10.0, y: 10.0, w: 10.0, h: 10.0 });
}

#[test]
fn a_broken_document_has_no_prompts() {
    assert!(prompts(r#"<svg viewBox="0 0 10 10"><g onclick="sendPrompt('a')"><rect"#).is_empty());
}
