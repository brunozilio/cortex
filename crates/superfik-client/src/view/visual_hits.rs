//! Where an agent's visuals can be clicked: each part that sends a prompt,
//! placed where its visual was painted.

use superfik_core::{Block, visual};
use superfik_render::Rect;

use super::target::Target;
use crate::state::VisualPrompt;

pub fn visual_prompts(blocks: &[Block], painted: &[Rect], turn: usize, clickable: &mut Vec<(Target, Rect)>) {
    let finished = blocks.iter().filter_map(|block| match block {
        Block::Visual { svg, complete } => Some((svg, *complete)),
        _ => None,
    });
    for (index, ((svg, _), rect)) in finished.zip(painted).filter(|((_, complete), _)| *complete).enumerate() {
        let Some(frame) = visual::frame(svg) else { continue };
        let scale = rect.w / frame.width;
        for (prompt, part) in visual::prompts(svg).into_iter().enumerate() {
            let area = part.area;
            let at = Rect::new(rect.x + area.x * scale, rect.y + area.y * scale, area.w * scale, area.h * scale);
            clickable.push((Target::VisualPrompt(VisualPrompt { turn, visual: index, prompt }), at));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_scale_with_the_painted_visual_and_skip_unfinished_ones() {
        let svg = r#"<svg viewBox="0 0 440 100"><rect onclick="sendPrompt('a')" x="40" y="20" width="80" height="40"/></svg>"#;
        let blocks = Block::parse(&format!("```visual\n{svg}\n```\n```visual\n{svg}"));
        let painted = [Rect::new(10.0, 100.0, 220.0, 50.0), Rect::new(10.0, 200.0, 220.0, 50.0)];
        let mut clickable = Vec::new();
        visual_prompts(&blocks, &painted, 2, &mut clickable);
        let at = VisualPrompt { turn: 2, visual: 0, prompt: 0 };
        assert_eq!(clickable, vec![(Target::VisualPrompt(at), Rect::new(30.0, 110.0, 40.0, 20.0))]);
    }
}
