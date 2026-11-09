//! Agent node: one sub-agent and its progress.

use superfik_core::{AgentRole, AgentStatus, SubAgent};
use superfik_i18n::Text;
use superfik_render::{Icon, Rect};
use superfik_ui::components::{AgentCard, AgentCardStatus, EarSide, NodeFrame};
use superfik_ui::{Painter, icons};

use super::{NodeCtx, node_height, reveal};
use crate::view::layout::NodeKey;
use crate::view::target::Target;

pub fn height() -> f32 {
    node_height(AgentCard::HEIGHT)
}

fn role_icon(role: AgentRole) -> Icon {
    match role {
        AgentRole::Explorer => icons::MAP,
        AgentRole::Builder => icons::CUBE,
        AgentRole::Reviewer => icons::SEARCH,
        AgentRole::Tester => icons::FLASK,
    }
}

pub fn paint(p: &mut Painter, ctx: &mut NodeCtx, agent: &SubAgent, open: bool) {
    let title = ctx.strings.t(Text::AgentTitle(agent.index + 1));
    let key = NodeKey::Agent(agent.index);
    let mut frame = NodeFrame::new(ctx.rect, icons::ROBOT, Some(p.palette().agent), &title);
    frame.ear = Some(EarSide::Right);
    frame.ear_hovered = ctx.is_hovered(Target::Close(key));
    frame.content = ctx.content;
    frame.time = ctx.time;
    frame.busy = ctx.busy;
    let hits = frame.paint(p);
    if let Some(knob) = hits.knob {
        ctx.hits.add(knob, Target::Close(key));
    }
    let seconds;
    let status = match &agent.status {
        AgentStatus::Running { .. } => AgentCardStatus::Running,
        AgentStatus::Done { seconds: s } => {
            seconds = format!("{s}s");
            AgentCardStatus::Done(&seconds)
        }
        other => AgentCardStatus::Label(ctx.strings.agent_status(other).unwrap_or_default()),
    };
    let body = ctx.body();
    let well = Rect::new(body.x - 1.0, body.y, body.w + 2.0, AgentCard::HEIGHT);
    let card = AgentCard {
        icon: if agent.custom.is_some() { icons::BOLT } else { role_icon(agent.role) },
        title: ctx.strings.agent_label(agent),
        summary: ctx.strings.agent_summary(agent),
        status,
        progress: agent.status.progress(),
        time: ctx.time,
        hovered: ctx.is_hovered(Target::Agent(agent.index)),
        open,
    };
    reveal(p, ctx.content, |p| card.paint(p, well));
    if agent.transcript.is_some() {
        ctx.hits.add(well, Target::Agent(agent.index));
    }
    if open {
        ctx.hits.open_agent_node = Some(ctx.rect);
    }
}
