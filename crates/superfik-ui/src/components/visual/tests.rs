use superfik_render::pictures::{faces, rasterize};

use super::*;
use crate::theme::colors::{DARK, LIGHT};

const NODE: &str = r##"<svg width="100%" viewBox="0 0 40 20">
<g class="node c-blue"><rect x="0" y="0" width="40" height="20" stroke-width="0.5"/></g>
<line x1="0" y1="0" x2="1" y2="1" stroke="var(--t)"/>
</svg>"##;

fn pixel(svg: &str, x: usize, y: usize) -> [u8; 4] {
    let pixels = rasterize(svg, &faces(), 40, 20).expect("raster");
    let at = (y * 40 + x) * 4;
    [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
}

#[test]
fn the_root_is_sized_by_its_view_box_and_named_as_svg() {
    let themed = themed(NODE, &LIGHT.visual);
    let root = &themed[..themed.find('>').expect("root")];
    assert!(!root.contains("width="), "{root}");
    assert!(root.contains(r#"viewBox="0 0 40 20""#));
    assert!(root.contains(r#"xmlns="http://www.w3.org/2000/svg""#));
    assert!(themed.contains("<style>"));
}

#[test]
fn ramp_classes_take_the_themes_stops() {
    assert_eq!(pixel(&themed(NODE, &LIGHT.visual), 20, 10), [0xE6, 0xF1, 0xFB, 255], "blue 50");
    assert_eq!(pixel(&themed(NODE, &DARK.visual), 20, 10), [0x0C, 0x44, 0x7C, 255], "blue 800");
}

#[test]
fn variables_become_the_themes_colours() {
    let themed = themed(NODE, &DARK.visual);
    assert!(!themed.contains("var("));
    assert!(themed.contains(&format!(r#"stroke="{}""#, css(DARK.visual.text_tertiary))));
    let unknown = resolve_variables("fill=\"var(--nope, #123456)\" x=\"var(--gone)\"", &LIGHT.visual);
    assert_eq!(unknown, format!("fill=\"#123456\" x=\"{}\"", css(LIGHT.visual.text)));
    assert_eq!(resolve_variables("var(--color-text-danger)", &LIGHT.visual), css(visual::RED.stops[stop::S600]));
}

#[test]
fn only_whole_attributes_are_stripped() {
    assert_eq!(
        strip_attribute(r#"<svg width="1" stroke-width="2" height='3'"#, "width"),
        r#"<svg stroke-width="2" height='3'"#
    );
}

#[test]
fn a_visual_holds_its_place_until_its_view_box_arrives() {
    let pending = Visual { svg: "<svg", ratio: None, complete: false, width: 440.0 };
    assert_eq!(pending.height(), visual::PENDING_HEIGHT);
    assert_eq!(Visual { ratio: Some(0.5), ..pending }.height(), 220.0);
}
