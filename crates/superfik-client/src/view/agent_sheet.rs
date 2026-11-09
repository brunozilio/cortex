//! A sub-agent's session: grows out of its card like the pull request sheet
//! grows out of the Repository node, and shows what the main agent asked
//! it, each step it takes and its answer, laid out like the conversation.
//! Read-only. Painted in window coordinates over everything.

use std::collections::{HashMap, HashSet};

use superfik_i18n::Text;
use superfik_render::{GooShape, Point, Rect};
use superfik_ui::components::{EarSide, NodeFrame, Sheet, Side};
use superfik_ui::theme::metrics;
use superfik_ui::{Painter, Plane, icons};

use super::motions::{Motions, SheetKind, WordsScope};
use super::target::{HitMap, Target};
use super::transcript::{self, TranscriptProps};
use crate::state::Session;

const STEM: f32 = 14.0;
const MARGIN: f32 = 8.0;
pub struct AgentSheetView<'a> {
    pub session: &'a Session,
    pub hovered: Option<Target>,
    pub viewport: (f32, f32),
    pub now: f64,
}

impl AgentSheetView<'_> {
    pub fn paint(&self, p: &mut Painter, motions: &mut Motions, from: Option<Rect>, hits: &mut HitMap) -> Option<Rect> {
        let c = self.session.conversation();
        let agent = c.opened_agent();
        motions.sync_sheet(SheetKind::Agent, agent.is_some(), self.now);
        let presence = motions.sheet(SheetKind::Agent, self.now)?;
        let agent = agent?;
        let turn = agent.transcript.as_ref()?;
        let strings = self.session.strings();
        let viewport = Rect::new(0.0, 0.0, self.viewport.0, self.viewport.1);
        let (anchor, side, panel, source) = place(from, viewport);
        let title = format!("{} · {}", strings.t(Text::AgentTitle(agent.index + 1)), strings.agent_label(agent));
        let sheet = Sheet {
            anchor,
            side,
            panel,
            source,
            progress: presence.progress(self.now),
            icon: icons::ROBOT,
            tint: Some(p.palette().agent),
            title: &title,
            close_hovered: self.hovered == Some(Target::CloseAgent),
        };
        let no_images = HashMap::new();
        let no_outputs = HashSet::new();
        let mut scroll = (Rect::default(), 0.0);
        let painted = p.on_plane(Plane::Overlay, |p| {
            sheet.paint(p, |p, body| {
                let mut props = TranscriptProps {
                    turns: std::slice::from_ref(turn),
                    live_since: c.agent_live_since(agent.index),
                    images: &no_images,
                    x: body.x,
                    bottom: body.bottom(),
                    fly_from: None,
                    words: WordsScope { conversation: c.id, agent: Some(agent.index) },
                    now: self.now,
                    root: self.session.root.as_deref(),
                    steps_open: &[Some(0.0)],
                    strings,
                    retry: false,
                    hovered: None,
                    utc_offset: self.session.clock.utc_offset_secs,
                    open_outputs: &no_outputs,
                    comments: &[],
                    sends: false,
                };
                let height = transcript::height(p, &props);
                let max = (height - body.h).max(0.0);
                props.bottom = body.y + height.min(body.h) + c.agent_scroll.min(max);
                p.clipped(body, |p| transcript::paint(p, &props, motions, 0.0, &mut Vec::new()));

                scroll = (body, max);
            })
        });
        if presence.is_visible() && painted.content_opacity > 0.6 {
            hits.add_screen(painted.panel, Target::Surface);
            hits.add_screen(painted.close, Target::CloseAgent);
            hits.agent_sheet = Some(scroll);
        }
        presence.is_visible().then_some(panel)
    }
}

fn place(from: Option<Rect>, viewport: Rect) -> (Point, Side, Rect, Option<GooShape>) {
    let (w, h) = (metrics::AGENT_SHEET_WIDTH, metrics::AGENT_SHEET_HEIGHT);
    let Some(card) = from else {
        let c = viewport.center();
        return (Point::new(c.x, c.y + h * 0.5 + STEM), Side::Up, Rect::new(c.x - w * 0.5, c.y - h * 0.5, w, h), None);
    };
    let cy = card.y + card.h * 0.5;
    let y = cy.clamp(h * 0.5 + MARGIN, (viewport.bottom() - h * 0.5 - MARGIN).max(h * 0.5 + MARGIN)) - h * 0.5;
    let ear = NodeFrame::ear_rect(card, EarSide::Right);
    let source = Some(GooShape::RoundRect { rect: card, radius: metrics::NODE_RADIUS });
    let right = ear.right() + STEM;
    if right + w <= viewport.right() - MARGIN {
        (Point::new(card.right(), cy), Side::Right, Rect::new(right, y, w, h), source)
    } else {
        (Point::new(card.x, cy), Side::Left, Rect::new(card.x - STEM - w, y, w, h), source)
    }
}

#[cfg(test)]
mod tests {
    use superfik_core::{EngineEvent, ToolCall};
    use superfik_protocol::ServerEvent;
    use superfik_render::Scene;
    use superfik_ui::painter::LAYERS;

    use super::*;
    use crate::state::{Action, EditKey};

    const VIEWPORT: (f32, f32) = (1400.0, 900.0);

    fn paint(session: &Session, motions: &mut Motions, from: Option<Rect>, now: f64) -> (HitMap, Option<Rect>) {
        let mut scene = Scene::new(superfik_render::Color::TRANSPARENT, LAYERS);
        let mut text = superfik_render::TextSystem::new();
        let mut p = Painter::new(&mut scene, &mut text, 1.0, &superfik_ui::theme::colors::DARK);
        let mut hits = HitMap::default();
        let view = AgentSheetView { session, hovered: None, viewport: VIEWPORT, now };
        let panel = view.paint(&mut p, motions, from, &mut hits);
        (hits, panel)
    }

    fn with_agent() -> Session {
        let mut s = Session::new(Some("/r".into()));
        s.apply(Action::Edit(EditKey::Insert("go".into())), 0.0);
        s.apply(Action::Submit, 0.0);
        let run = s.conversation().run.as_ref().map(|r| r.id).expect("running");
        let event = |event| Action::Server(ServerEvent::Run { run, event });
        let spawned = EngineEvent::AgentSpawned {
            index: 0,
            kind: "Explore".into(),
            summary: "Map".into(),
            prompt: "Map it".into(),
        };
        s.apply(event(spawned), 1.0);
        let step = EngineEvent::ToolStarted(ToolCall::new("s1", "Read", "/r/src/main.rs"));
        s.apply(event(EngineEvent::AgentEvent { index: 0, event: Box::new(step) }), 1.1);
        s
    }

    #[test]
    fn opens_beside_the_card_with_a_close_knob_and_the_wheel_routed_to_it() {
        let mut s = with_agent();
        let mut motions = Motions::default();
        let card = Rect::new(600.0, 300.0, 216.0, 100.0);
        assert!(paint(&s, &mut motions, Some(card), 1.0).1.is_none(), "closed");
        s.apply(Action::ToggleAgent(0), 1.0);
        paint(&s, &mut motions, Some(card), 1.0);
        let (hits, panel) = paint(&s, &mut motions, Some(card), 4.0);
        let panel = panel.expect("open");
        assert!(panel.x > card.right(), "opens to the card's right");
        assert!(hits.agent_sheet.is_some());
        let centre = panel.center();
        assert_eq!(hits.at(centre, centre), Some(Target::Surface), "the canvas under it is inert");
        s.apply(Action::CloseAgent, 4.0);
        paint(&s, &mut motions, Some(card), 4.0);
        assert!(paint(&s, &mut motions, Some(card), 8.0).0.agent_sheet.is_none());
    }

    #[test]
    fn opens_to_the_left_when_the_window_ends_on_the_right() {
        let viewport = Rect::new(0.0, 0.0, VIEWPORT.0, VIEWPORT.1);
        let card = Rect::new(1100.0, 300.0, 216.0, 100.0);
        let (_, side, panel, _) = place(Some(card), viewport);
        assert_eq!(side, Side::Left);
        assert!(panel.right() < card.x && panel.x >= 0.0);
        let (_, side, panel, _) = place(None, viewport);
        assert_eq!(side, Side::Up);
        assert!(viewport.contains(panel.center()));
    }
}
