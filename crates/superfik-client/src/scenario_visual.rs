//! An agent's reply with a visual: a small flowchart drawn inline, with
//! clickable nodes; or the same reply while the drawing streams in.

use superfik_core::Activity;

use crate::scenarios::{SyntheticImage, done};
use crate::state::{ActiveRun, Session};

const FLOW: &str = r##"<svg width="100%" viewBox="0 0 440 280">
<defs><marker id="arrow" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse"><path d="M2 1L8 5L2 9" fill="none" stroke="context-stroke" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></marker></defs>
<g class="node" onclick="sendPrompt('What goes into the prompt?')">
  <rect class="box" x="140" y="20" width="160" height="44" rx="8" stroke-width="0.5"/>
  <text class="th" x="220" y="42" text-anchor="middle" dominant-baseline="central">Prompt</text>
</g>
<line x1="220" y1="64" x2="220" y2="96" class="arr" marker-end="url(#arrow)"/>
<g class="node c-purple" onclick="sendPrompt('How does the agent loop decide the next step?')">
  <rect x="110" y="100" width="220" height="56" rx="8" stroke-width="0.5"/>
  <text class="th" x="220" y="119" text-anchor="middle" dominant-baseline="central">Agent loop</text>
  <text class="ts" x="220" y="139" text-anchor="middle" dominant-baseline="central">Plans each step</text>
</g>
<path d="M170 156 L170 176 L105 176 L105 196" fill="none" class="arr" marker-end="url(#arrow)"/>
<path d="M270 156 L270 176 L335 176 L335 196" fill="none" class="arr" marker-end="url(#arrow)"/>
<g class="node c-teal" onclick="sendPrompt('Which tools can the agent use?')">
  <rect x="30" y="200" width="150" height="56" rx="8" stroke-width="0.5"/>
  <text class="th" x="105" y="219" text-anchor="middle" dominant-baseline="central">Tools</text>
  <text class="ts" x="105" y="239" text-anchor="middle" dominant-baseline="central">Read, edit, run</text>
</g>
<g class="node c-teal" onclick="sendPrompt('How is the reply streamed?')">
  <rect x="260" y="200" width="150" height="56" rx="8" stroke-width="0.5"/>
  <text class="th" x="335" y="219" text-anchor="middle" dominant-baseline="central">Reply</text>
  <text class="ts" x="335" y="239" text-anchor="middle" dominant-baseline="central">Streams to you</text>
</g>
</svg>"##;

pub fn visual(complete: bool) -> (Session, Vec<SyntheticImage>) {
    let (mut s, images) = done();
    let c = s.conversation_mut();
    let Some(turn) = c.turns.last_mut() else { return (s, images) };
    let intro = "Each prompt goes through the agent loop, which picks a tool or answers.";
    turn.response.text = if complete {
        format!("{intro}\n\n```visual\n{FLOW}\n```\n\nClick a step to dig into it.")
    } else {
        turn.response.activity = Activity::Writing;
        let cut = FLOW.find("<g class=\"node c-teal\"").unwrap_or(FLOW.len());
        format!("{intro}\n\n```visual\n{}", &FLOW[..cut])
    };
    if !complete {
        c.finished_at = None;
        c.run = Some(ActiveRun { id: superfik_core::RunId::new(), started_at: 0.0 });
    }
    (s, images)
}

#[cfg(test)]
mod tests {
    use superfik_core::{Block, visual};

    use super::*;

    #[test]
    fn the_flowchart_has_four_clickable_steps() {
        assert_eq!(visual::prompts(FLOW).len(), 4);
        let (s, _) = super::visual(true);
        let blocks = s.conversation().turns.last().map(|t| t.response.blocks()).unwrap_or_default();
        assert!(blocks.iter().any(|b| matches!(b, Block::Visual { complete: true, .. })));
    }
}
