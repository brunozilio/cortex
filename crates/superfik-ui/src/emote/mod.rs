//! The emote: Superfik's face. A ring of light with two eyes that blink,
//! glance and react, the same mark the app icon is made of. Pure geometry
//! over `(state, time)` in a unit box (ring width 1, origin at the ring's
//! centre, y down); the component scales and colours it.

mod raster;

use superfik_motion::Easing;
use superfik_render::Color;

use crate::theme::motion;

pub use raster::{Look, raster};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EmoteState {
    Idle,
    Thinking,
    Alert,
    Question,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub eye_w: f32,
    pub eye_h: f32,
    pub eye_gap: f32,
    pub look_x: f32,
    pub look_y: f32,
    pub tilt: f32,
    pub smile: f32,
    pub alert: f32,
    pub flow: f32,
}

impl Pose {
    fn of(state: EmoteState) -> Self {
        let base = Pose {
            eye_w: 0.15,
            eye_h: 0.27,
            eye_gap: 0.40,
            look_x: 0.0,
            look_y: 0.0,
            tilt: 0.0,
            smile: 0.0,
            alert: 0.0,
            flow: 0.15,
        };
        match state {
            EmoteState::Idle => base,
            EmoteState::Thinking => Pose { eye_h: 0.21, look_y: -0.05, flow: 1.0, ..base },
            EmoteState::Alert => Pose { eye_w: 0.16, eye_h: 0.32, alert: 1.0, flow: 0.3, ..base },
            EmoteState::Question => Pose { tilt: 0.07, look_x: 0.04, look_y: -0.02, flow: 0.25, ..base },
            EmoteState::Done => Pose { smile: 1.0, flow: 0.45, ..base },
        }
    }

    fn lerp(a: Self, b: Self, k: f32) -> Self {
        let l = |x: f32, y: f32| x + (y - x) * k;
        Pose {
            eye_w: l(a.eye_w, b.eye_w),
            eye_h: l(a.eye_h, b.eye_h),
            eye_gap: l(a.eye_gap, b.eye_gap),
            look_x: l(a.look_x, b.look_x),
            look_y: l(a.look_y, b.look_y),
            tilt: l(a.tilt, b.tilt),
            smile: l(a.smile, b.smile),
            alert: l(a.alert, b.alert),
            flow: l(a.flow, b.flow),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dot {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub stop: f32,
    pub a: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eye {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub smile: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    Exclamation,
    Question,
    Thumb,
}

impl Mark {
    fn of(state: EmoteState) -> Option<Self> {
        match state {
            EmoteState::Alert => Some(Mark::Exclamation),
            EmoteState::Question => Some(Mark::Question),
            EmoteState::Done => Some(Mark::Thumb),
            EmoteState::Idle | EmoteState::Thinking => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EmoteFrame {
    pub ring: Vec<Dot>,
    pub eyes: [Eye; 2],
    pub marks: Vec<(Mark, f32)>,
    pub alert: f32,
    pub pulse: f32,
}

pub fn ring_color(stops: &[Color; 12], stop: f32) -> Color {
    let x = stop.rem_euclid(1.0) * 12.0;
    let i = x.floor() as usize % 12;
    stops[i].mix(stops[(i + 1) % 12], x - x.floor())
}

pub const RING_W: f32 = 1.0;
pub const RING_H: f32 = 0.86;
pub const RING_R: f32 = 0.34;
pub const CORE: f32 = 0.052;
pub const TRAIL: f32 = 0.017;
const CORE_DOTS: usize = 96;
const TRAIL_DOTS: usize = 200;

pub fn ring_pt(t: f32, offset: f32) -> (f32, f32) {
    let (w, h, r) = (RING_W, RING_H, RING_R);
    let (sw, sh) = (w - 2.0 * r, h - 2.0 * r);
    let arc = std::f32::consts::FRAC_PI_2 * r;
    let perimeter = 2.0 * sw + 2.0 * sh + 4.0 * arc;
    let mut d = t.rem_euclid(1.0) * perimeter;
    let corners = [(sw * 0.5, -sh * 0.5), (sw * 0.5, sh * 0.5), (-sw * 0.5, sh * 0.5), (-sw * 0.5, -sh * 0.5)];
    let edges = [
        ((-sw * 0.5, -h * 0.5), (1.0, 0.0), sw, (0.0, -1.0)),
        ((w * 0.5, -sh * 0.5), (0.0, 1.0), sh, (1.0, 0.0)),
        ((sw * 0.5, h * 0.5), (-1.0, 0.0), sw, (0.0, 1.0)),
        ((-w * 0.5, sh * 0.5), (0.0, -1.0), sh, (-1.0, 0.0)),
    ];
    for (i, (start, dir, len, normal)) in edges.iter().enumerate() {
        if d <= *len {
            return (start.0 + dir.0 * d + normal.0 * offset, start.1 + dir.1 * d + normal.1 * offset);
        }
        d -= len;
        if d <= arc {
            let a0 = std::f32::consts::FRAC_PI_2 * (i as f32 - 1.0);
            let a = a0 + d / r;
            let (cx, cy) = corners[i];
            return (cx + (r + offset) * a.cos(), cy + (r + offset) * a.sin());
        }
        d -= arc;
    }
    let (cx, cy) = corners[3];
    (cx - r - offset, cy)
}

fn stop_of(x: f32, y: f32) -> f32 {
    (y.atan2(x) / std::f32::consts::TAU).rem_euclid(1.0)
}

fn ring(pose: &Pose, time: f32) -> Vec<Dot> {
    let mut dots = Vec::with_capacity(CORE_DOTS + 2 * TRAIL_DOTS);
    for i in 0..CORE_DOTS {
        let t = i as f32 / CORE_DOTS as f32;
        let (x, y) = ring_pt(t, 0.0);
        dots.push(Dot { x, y, r: CORE, stop: stop_of(x, y), a: 1.0 });
    }
    let phase = time * (0.35 + 1.4 * pose.flow);
    for (loop_i, (freq, amp, speed)) in [(2.0, 0.075, 1.0), (3.0, 0.06, -0.7)].into_iter().enumerate() {
        for i in 0..TRAIL_DOTS {
            let t = i as f32 / TRAIL_DOTS as f32;
            let wave = (std::f32::consts::TAU * (freq * t + speed * phase * 0.2) + loop_i as f32 * 2.1).sin();
            let (x, y) = ring_pt(t, amp * wave * (0.35 + 0.65 * pose.flow) + CORE * 0.6);
            dots.push(Dot { x, y, r: TRAIL, stop: stop_of(x, y), a: 0.35 + 0.5 * pose.flow });
        }
    }
    dots
}

pub fn blink(time: f32) -> f32 {
    let phase = (time + 1.7).rem_euclid(motion::EMOTE_BLINK_PERIOD);
    if phase > motion::EMOTE_BLINK {
        return 0.0;
    }
    (std::f32::consts::PI * phase / motion::EMOTE_BLINK).sin()
}

fn glance(state: EmoteState, time: f32) -> (f32, f32) {
    match state {
        EmoteState::Thinking => {
            let swing = (time * 0.9).sin().clamp(-0.6, 0.6) / 0.6;
            (0.07 * swing, -0.02 * (time * 2.3).sin())
        }
        EmoteState::Idle | EmoteState::Question => {
            (0.035 * (time * 0.7).sin() + 0.02 * (time * 1.9).sin(), 0.02 * (time * 1.1).sin())
        }
        EmoteState::Alert | EmoteState::Done => (0.0, 0.0),
    }
}

pub fn frame(state: EmoteState, from: Option<(EmoteState, f32)>, time: f32) -> EmoteFrame {
    let (pose, marks) = match from.filter(|(_, k)| *k < 1.0) {
        Some((previous, k)) => {
            let k = Easing::IN_OUT.apply(k);
            let mut marks = Vec::new();
            if let Some(m) = Mark::of(previous) {
                marks.push((m, 1.0 - k));
            }
            if let Some(m) = Mark::of(state) {
                marks.push((m, k));
            }
            (Pose::lerp(Pose::of(previous), Pose::of(state), k), marks)
        }
        None => (Pose::of(state), Mark::of(state).map(|m| (m, 1.0)).into_iter().collect()),
    };
    let (gx, gy) = glance(state, time);
    let shut = blink(time) * (1.0 - pose.smile);
    let eye_h = pose.eye_h * (1.0 - 0.92 * shut);
    let (lx, ly) = (pose.look_x + gx, pose.look_y + gy);
    let eye = |side: f32| Eye {
        x: side * pose.eye_gap * 0.5 + lx,
        y: ly - side.max(0.0) * pose.tilt + side.min(0.0) * pose.tilt * 0.3,
        w: pose.eye_w,
        h: eye_h.max(pose.eye_w * 0.35),
        smile: pose.smile,
    };
    EmoteFrame {
        ring: ring(&pose, time),
        eyes: [eye(-1.0), eye(1.0)],
        marks,
        alert: pose.alert,
        pulse: 0.5 + 0.5 * (time * 6.0).sin(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_closes_and_stays_in_its_box() {
        let (x0, y0) = ring_pt(0.0, 0.0);
        let (x1, y1) = ring_pt(1.0, 0.0);
        assert!((x0 - x1).abs() < 1e-4 && (y0 - y1).abs() < 1e-4);
        for i in 0..200 {
            let (x, y) = ring_pt(i as f32 / 200.0, 0.0);
            assert!(x.abs() <= RING_W * 0.5 + 1e-4 && y.abs() <= RING_H * 0.5 + 1e-4, "{i}: {x} {y}");
        }
    }

    #[test]
    fn ring_runs_clockwise_from_the_top_edge() {
        let (x, y) = ring_pt(0.05, 0.0);
        assert!(y < 0.0 && x < 0.0, "starts on the top edge, left half");
        let (x, _) = ring_pt(0.25, 0.0);
        assert!(x > 0.3, "a quarter turn later it is on the right");
    }

    #[test]
    fn stops_cover_the_whole_hue_circle() {
        let f = frame(EmoteState::Idle, None, 0.0);
        let core = &f.ring[..CORE_DOTS];
        let (min, max) = core.iter().fold((1.0f32, 0.0f32), |(lo, hi), d| (lo.min(d.stop), hi.max(d.stop)));
        assert!(min < 0.02 && max > 0.98);
    }

    #[test]
    fn idle_blinks_and_opens_again() {
        let open = frame(EmoteState::Idle, None, 1.0).eyes[0].h;
        let mid_blink = motion::EMOTE_BLINK_PERIOD - 1.7 + motion::EMOTE_BLINK * 0.5;
        let shut = frame(EmoteState::Idle, None, mid_blink).eyes[0].h;
        assert!(shut < open * 0.4, "shut {shut} open {open}");
        assert!((frame(EmoteState::Idle, None, mid_blink + 1.0).eyes[0].h - open).abs() < 0.03);
    }

    #[test]
    fn states_have_their_marks_and_shapes() {
        assert!(frame(EmoteState::Idle, None, 0.0).marks.is_empty());
        assert_eq!(frame(EmoteState::Alert, None, 0.0).marks, vec![(Mark::Exclamation, 1.0)]);
        assert_eq!(frame(EmoteState::Question, None, 0.0).marks, vec![(Mark::Question, 1.0)]);
        assert_eq!(frame(EmoteState::Done, None, 0.0).marks, vec![(Mark::Thumb, 1.0)]);
        let q = frame(EmoteState::Question, None, 0.0);
        assert!(q.eyes[1].y < q.eyes[0].y, "the right eye is raised");
        let done = frame(EmoteState::Done, None, 0.0);
        assert_eq!(done.eyes[0].smile, 1.0);
        assert!(frame(EmoteState::Thinking, None, 0.0).eyes[0].h < frame(EmoteState::Idle, None, 0.0).eyes[0].h);
    }

    #[test]
    fn morph_blends_poses_and_cross_fades_marks() {
        let half = frame(EmoteState::Alert, Some((EmoteState::Done, 0.5)), 0.0);
        assert!(half.alert > 0.3 && half.alert < 0.7);
        assert!(half.eyes[0].smile > 0.3 && half.eyes[0].smile < 0.7);
        assert_eq!(half.marks.len(), 2);
        assert!((half.marks[0].1 + half.marks[1].1 - 1.0).abs() < 1e-5);
        let done = frame(EmoteState::Alert, Some((EmoteState::Done, 1.0)), 0.0);
        assert_eq!(done, frame(EmoteState::Alert, None, 0.0));
    }

    #[test]
    fn thinking_flows_more_than_idle() {
        let idle = frame(EmoteState::Idle, None, 2.0);
        let busy = frame(EmoteState::Thinking, None, 2.0);
        assert!(busy.ring[CORE_DOTS].a > idle.ring[CORE_DOTS].a);
        assert_ne!(busy.ring[CORE_DOTS], frame(EmoteState::Thinking, None, 2.5).ring[CORE_DOTS]);
    }
}
