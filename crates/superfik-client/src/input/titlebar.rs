//! The invisible title bar: the strip along the top of the window drags it,
//! and a double-click there maximizes it (or restores it), unless the user chose
//! to minimise or do nothing in System Settings.

use superfik_render::Point;

pub const HEIGHT: f32 = superfik_ui::theme::metrics::TITLEBAR_HEIGHT;
const DOUBLE_CLICK_SECONDS: f64 = 0.5;
const DOUBLE_CLICK_SLOP: f32 = 5.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleClickAction {
    Maximize,
    Minimize,
    None,
}

impl DoubleClickAction {
    pub fn parse(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            Some("Minimize") => Self::Minimize,
            Some("None") => Self::None,
            _ => Self::Maximize,
        }
    }

    pub fn from_system() -> Self {
        #[cfg(not(target_os = "macos"))]
        return Self::Maximize;
        #[cfg(target_os = "macos")]
        {
            let output =
                std::process::Command::new("defaults").args(["read", "-g", "AppleActionOnDoubleClick"]).output().ok();
            let value = output.filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
            Self::parse(value.as_deref())
        }
    }
}

pub fn contains(point: Point) -> bool {
    point.y >= 0.0 && point.y < HEIGHT
}

#[derive(Debug, Default)]
pub struct ClickCounter {
    last: Option<(f64, Point)>,
}

impl ClickCounter {
    pub fn press(&mut self, time: f64, at: Point) -> bool {
        let double =
            self.last.is_some_and(|(t, p)| time - t <= DOUBLE_CLICK_SECONDS && p.distance(at) <= DOUBLE_CLICK_SLOP);
        self.last = if double { None } else { Some((time, at)) };
        double
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_preference_maps_to_actions() {
        assert_eq!(DoubleClickAction::parse(None), DoubleClickAction::Maximize);
        assert_eq!(DoubleClickAction::parse(Some("Maximize\n")), DoubleClickAction::Maximize);
        assert_eq!(DoubleClickAction::parse(Some("Fill")), DoubleClickAction::Maximize);
        assert_eq!(DoubleClickAction::parse(Some("Minimize")), DoubleClickAction::Minimize);
        assert_eq!(DoubleClickAction::parse(Some("None")), DoubleClickAction::None);
    }

    #[test]
    fn two_quick_nearby_clicks_are_a_double_click() {
        let mut clicks = ClickCounter::default();
        assert!(!clicks.press(1.0, Point::new(10.0, 10.0)));
        assert!(clicks.press(1.3, Point::new(12.0, 11.0)));
        assert!(!clicks.press(1.4, Point::new(12.0, 11.0)), "a third click starts over");
    }

    #[test]
    fn slow_or_distant_clicks_are_not() {
        let mut clicks = ClickCounter::default();
        clicks.press(1.0, Point::new(10.0, 10.0));
        assert!(!clicks.press(2.0, Point::new(10.0, 10.0)));
        assert!(!clicks.press(2.1, Point::new(40.0, 10.0)));
    }

    #[test]
    fn only_the_top_strip_is_the_title_bar() {
        assert!(contains(Point::new(500.0, 4.0)));
        assert!(!contains(Point::new(500.0, HEIGHT + 1.0)));
    }
}
