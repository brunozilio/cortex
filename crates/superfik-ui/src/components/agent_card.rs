//! The card inside an agent node: role tile, status and dashed progress.

use superfik_render::{Fill, Icon, Pattern, Point, Quad, Rect};

use crate::icons;
use crate::painter::Painter;
use crate::theme::{colors, metrics, typography};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AgentCardStatus<'a> {
    Label(&'a str),
    Running,
    Done(&'a str),
}

#[derive(Debug, Clone, Copy)]
pub struct AgentCard<'a> {
    pub icon: Icon,
    pub title: &'a str,
    pub summary: &'a str,
    pub status: AgentCardStatus<'a>,
    pub progress: f32,
    pub time: f32,
    pub hovered: bool,
    pub open: bool,
}

impl AgentCard<'_> {
    pub const HEIGHT: f32 = 66.0;
    const DASHES: usize = 10;

    pub fn paint(&self, p: &mut Painter, well: Rect) {
        let c = p.palette();
        let fill = match (self.open, self.hovered) {
            (true, _) => c.control_active,
            (false, true) => c.hover,
            (false, false) => c.well,
        };
        p.quad(Quad::new(well, metrics::WELL_RADIUS, fill));
        let tile = Rect::new(well.x + 9.0, well.y + 10.0, 30.0, 30.0);
        p.quad(
            Quad::new(tile, metrics::ROLE_TILE_RADIUS, p.palette().role_tile_top)
                .fill(Fill::vertical(p.palette().role_tile_top, p.palette().role_tile_bottom))
                .pattern(Pattern::Grain { amount: colors::ROLE_TILE_GRAIN }),
        );
        p.icon_at(self.icon, tile.center(), metrics::ROLE_ICON, Some(p.palette().icon));

        let text_x = tile.right() + 11.0;
        let right = well.right() - 10.0;
        let status_width = self.paint_status(p, right, well.y + 17.0);
        p.label_fit(
            self.title,
            typography::TITLE,
            text_x,
            well.y + 17.0,
            right - status_width - text_x,
            p.palette().text,
        );
        p.label_fit(
            self.summary,
            typography::SUBTITLE,
            text_x,
            well.y + 34.0,
            right - status_width - text_x,
            p.palette().text_subtle,
        );
        self.paint_dashes(p, Rect::new(tile.x, well.y + 51.0, right - tile.x, 4.0));
    }

    fn paint_status(&self, p: &mut Painter, right: f32, cy: f32) -> f32 {
        match self.status {
            AgentCardStatus::Label(text) => {
                let layout = p.layout(text, typography::CAPTION, None);
                p.text(
                    &layout,
                    Point::new(right - layout.width, (cy - layout.height * 0.5).round()),
                    p.palette().text_tertiary,
                );
                layout.width + 8.0
            }
            AgentCardStatus::Done(time) => {
                let layout = p.layout(time, typography::CAPTION, None);
                let x = right - layout.width;
                p.text(&layout, Point::new(x, (cy - layout.height * 0.5).round()), p.palette().text_tertiary);
                p.icon_at(icons::CHECK_CIRCLE, Point::new(x - 12.5, cy), 12.0, Some(p.palette().agent));
                layout.width + 26.0
            }
            AgentCardStatus::Running => {
                let pulse = (self.time * 4.0).sin() * 0.5 + 0.5;
                let dot = Rect::new(right - 7.0, cy - 3.0, 6.0, 6.0);
                p.quad(Quad::new(dot, 3.0, p.palette().agent.faded(0.45 + 0.55 * pulse)));
                16.0
            }
        }
    }

    fn paint_dashes(&self, p: &mut Painter, track: Rect) {
        let gap = 4.6;
        let w = (track.w - gap * (Self::DASHES - 1) as f32) / Self::DASHES as f32;
        let lit = self.progress.clamp(0.0, 1.0) * Self::DASHES as f32;
        for i in 0..Self::DASHES {
            let rect = Rect::new(track.x + i as f32 * (w + gap), track.y, w, track.h);
            let fill = (lit - i as f32).clamp(0.0, 1.0);
            let running = matches!(self.status, AgentCardStatus::Running);
            let shimmer = if running && fill < 1.0 && fill > 0.0 { (self.time * 6.0).sin() * 0.15 } else { 0.0 };
            let color = p.palette().badge.mix(p.palette().agent_progress, (fill + shimmer).clamp(0.0, 1.0));
            p.quad(Quad::new(rect, track.h * 0.5, color));
        }
    }
}
