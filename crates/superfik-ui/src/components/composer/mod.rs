//! The composer: model header, input well and controls footer.

mod header;
mod highlight;
mod input;
mod shell;

use superfik_render::{Point, Rect};

pub use self::highlight::{Highlight, HighlightMark};
use super::chip::{Chip, SquareButton};
use super::send::SendButton;
pub use super::send::SendState;
use crate::icons;
use crate::painter::Painter;
use crate::text_edit::TextEdit;
use crate::theme::metrics;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerPart {
    Model,
    Effort,
    Mic,
    Image,
    Permission,
    Send,
}

#[derive(Debug, Clone, Copy)]
pub struct ComposerProps<'a> {
    pub origin: Point,
    pub model: &'a str,
    pub previous_model: Option<(&'a str, f32)>,
    pub effort: &'a str,
    pub effort_level: f32,
    pub ultra: bool,
    pub edit: &'a TextEdit,
    pub completion: Option<&'a str>,
    pub highlights: &'a [Highlight],
    pub placeholder: &'a str,
    pub caret_visible: bool,
    pub scroll: f32,
    pub follow_caret: bool,
    pub permission: (&'a str, bool),
    pub permission_unsafe: bool,
    pub send: SendState,
    pub mic: MicState,
    pub hovered: Option<ComposerPart>,
    pub time: f32,
}

#[derive(Debug, Clone, Default)]
pub struct ComposerHits {
    pub body: Rect,
    pub model: Rect,
    pub effort: Rect,
    pub effort_track: Rect,
    pub input: Rect,
    pub input_scroll: f32,
    pub input_overflows: bool,
    pub input_text: Option<(std::sync::Arc<superfik_render::TextLayout>, Point)>,
    pub mic: Rect,
    pub image: Rect,
    pub permission: Rect,
    pub send: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicState {
    Hidden,
    Idle,
    Watching,
    Live,
}

#[derive(Debug, Clone, Copy)]
pub struct Composer<'a> {
    pub props: ComposerProps<'a>,
}

impl Composer<'_> {
    pub const SIZE: (f32, f32) = (metrics::COMPOSER_WIDTH, metrics::COMPOSER_HEIGHT);

    pub fn paint(&self, p: &mut Painter) -> ComposerHits {
        let props = &self.props;
        let o = props.origin;
        let at = |x: f32, y: f32, w: f32, h: f32| Rect::new(o.x + x, o.y + y, w, h);
        let hovered = |part| props.hovered == Some(part);

        shell::paint_body(p, o);
        let (model, effort, effort_track) = header::paint(p, props);
        let (input, flow) = shell::paint_input_well(p, o);
        let scrolled = input::paint(p, props, input, flow);

        let mic = match props.mic {
            MicState::Hidden => Rect::default(),
            state => SquareButton {
                icon: icons::MIC,
                hovered: hovered(ComposerPart::Mic),
                lit: state == MicState::Live,
                dot: state == MicState::Watching,
            }
            .paint(p, at(11.0, 111.0, 30.0, 30.0)),
        };
        let image = SquareButton { icon: icons::IMAGE, hovered: hovered(ComposerPart::Image), lit: false, dot: false }
            .paint(p, at(11.0, 150.0, 30.0, 30.0));
        let permission = Chip {
            icon: icons::SHIELD,
            label: props.permission.0,
            active: props.permission.1,
            hovered: hovered(ComposerPart::Permission),
            tint: props.permission_unsafe.then_some(p.palette().attention),
        }
        .paint(p, Point::new(o.x + 46.0, o.y + 149.0));
        let send = SendButton { state: props.send, hovered: hovered(ComposerPart::Send) }
            .paint(p, at(405.5, 154.5, metrics::SEND_SIZE, metrics::SEND_SIZE));

        ComposerHits {
            body: at(0.0, 0.0, Self::SIZE.0, Self::SIZE.1),
            model,
            effort,
            effort_track,
            input,
            input_scroll: scrolled.scroll,
            input_overflows: scrolled.overflow > 0.0,
            input_text: scrolled.text,
            mic,
            image,
            permission,
            send,
        }
    }
}
