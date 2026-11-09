//! The expanded step list under a reply's status line, built from the
//! turn's domain steps. A tool step with an output opens on a click to show
//! what the tool returned; a failed one is marked.

use std::collections::HashSet;

use superfik_core::{Step, StepKind, Turn, TurnId};
use superfik_render::{Icon, Point, Rect};
use superfik_ui::components::{StepItem, StepList};
use superfik_ui::theme::motion;
use superfik_ui::{Painter, icons};

use super::target::Target;
use super::transcript::TranscriptProps;

const GAP: f32 = 8.0;

fn icon(kind: StepKind) -> Icon {
    match kind {
        StepKind::Read => icons::DOCUMENT,
        StepKind::Edit => icons::PENCIL,
        StepKind::Run => icons::PLAY,
        StepKind::Search => icons::SEARCH,
        StepKind::Web => icons::GLOBE,
        StepKind::Agent => icons::ROBOT,
        StepKind::Skill => icons::SPARKLE,
        StepKind::Reply => icons::CHAT,
        StepKind::Other => icons::MORE,
    }
}

fn shown<'a>(open: &HashSet<(TurnId, usize)>, turn: &Turn, steps: &'a [Step], index: usize) -> Option<&'a str> {
    let output = steps.get(index)?.output.as_deref()?;
    open.contains(&(turn.id, index)).then_some(output)
}

fn outputs<'a>(open: &HashSet<(TurnId, usize)>, turn: &Turn, steps: &'a [Step]) -> Vec<Option<&'a str>> {
    (0..steps.len()).map(|index| shown(open, turn, steps, index)).collect()
}

pub fn height(turn: &Turn, opened_at: Option<f64>, open: &HashSet<(TurnId, usize)>) -> f32 {
    match opened_at {
        Some(_) => {
            let steps = turn.response.step_list(None);
            GAP + outputs(open, turn, &steps).into_iter().map(StepList::row_height_of).sum::<f32>()
        }
        None => 0.0,
    }
}

pub fn reveal(opened_at: f64, now: f64, index: usize) -> f32 {
    let start = opened_at + index as f64 * f64::from(motion::STEP_STAGGER);
    motion::ENTER.apply((((now - start) as f32) / motion::STEP_REVEAL).clamp(0.0, 1.0))
}

#[allow(clippy::too_many_arguments)]
pub fn paint(
    p: &mut Painter,
    props: &TranscriptProps,
    turn: &Turn,
    index: usize,
    opened_at: Option<f64>,
    origin: Point,
    width: f32,
    clickable: &mut Vec<(Target, Rect)>,
) -> f32 {
    let Some(opened_at) = opened_at else { return 0.0 };
    let (strings, now) = (&props.strings, props.now);
    let steps = turn.response.step_list(props.root);
    let texts = outputs(props.open_outputs, turn, &steps);
    let items: Vec<StepItem> = steps
        .iter()
        .zip(&texts)
        .map(|(step, output)| StepItem {
            icon: icon(step.kind),
            verb: strings.step_verb(&step.verb),
            target: &step.target,
            failed: step.failed,
            output: *output,
        })
        .collect();
    let reveal = |row: usize| reveal(opened_at, now, row);
    let rows = StepList { items: &items, width, reveal: &reveal }.paint(p, Point::new(origin.x, origin.y + GAP));
    for (row, rect) in rows.into_iter().enumerate() {
        if row < turn.response.tools.len() && steps[row].output.is_some() {
            clickable.push((Target::StepOutput(index, row), rect));
        }
    }
    height(turn, Some(opened_at), props.open_outputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_resolve_in_order() {
        assert_eq!(reveal(1.0, 1.0, 0), 0.0);
        assert!(reveal(1.0, 1.1, 0) > reveal(1.0, 1.1, 1));
        assert_eq!(reveal(1.0, 5.0, 3), 1.0);
    }

    #[test]
    fn an_opened_output_adds_its_block_and_a_closed_one_adds_nothing() {
        let user = superfik_core::UserMessage { text: "run it".into(), images: vec![], branch_caption: String::new() };
        let mut turn = Turn::new(user);
        let mut call = superfik_core::ToolCall::new("1", "Bash", "cargo test");
        call.output = Some("ok\nfine".into());
        turn.response.tools.push(call);
        let closed = HashSet::new();
        let open = HashSet::from([(turn.id, 0)]);
        assert_eq!(height(&turn, Some(0.0), &closed), GAP + superfik_ui::components::StepList::row_height_of(None));
        assert!(height(&turn, Some(0.0), &open) > height(&turn, Some(0.0), &closed));
    }
}
