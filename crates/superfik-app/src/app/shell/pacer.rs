//! Frame pacing when presenting does not wait for the display.
//!
//! A focused window renders every refresh and lets presenting with vsync
//! pace the loop. Some systems never make it wait (Xvfb, some Linux
//! drivers, a software GPU): the loop then spins as fast as it can. The
//! pacer notices a run of frames much shorter than the display's refresh
//! and, from then on, has the loop wait for the next refresh on a timer.
//! Real vsync never trips it, so frames there keep vsync's even pacing.

const SHORT: f64 = 0.5;
const RUN: u32 = 12;
const DEFAULT_HZ: f64 = 60.0;

#[derive(Debug, Clone, Copy)]
pub struct Pacer {
    interval: f64,
    short_run: u32,
    unpaced: bool,
}

impl Default for Pacer {
    fn default() -> Self {
        Self { interval: 1.0 / DEFAULT_HZ, short_run: 0, unpaced: false }
    }
}

impl Pacer {
    pub fn set_refresh(&mut self, millihertz: Option<u32>) {
        let hz = millihertz.filter(|&mhz| mhz > 0).map_or(DEFAULT_HZ, |mhz| f64::from(mhz) / 1000.0);
        self.interval = 1.0 / hz;
    }

    pub fn observe(&mut self, frame: f64) {
        if self.unpaced {
            return;
        }
        self.short_run = if frame < self.interval * SHORT { self.short_run + 1 } else { 0 };
        if self.short_run >= RUN {
            self.unpaced = true;
            tracing::info!(interval_ms = self.interval * 1000.0, "presenting does not wait for vsync; pacing frames");
        }
    }

    pub fn next_frame(&self, last: f64) -> Option<f64> {
        self.unpaced.then_some(last + self.interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_paced_by_vsync_render_at_once() {
        let mut pacer = Pacer::default();
        for _ in 0..200 {
            pacer.observe(1.0 / 60.0);
        }
        assert_eq!(pacer.next_frame(1.0), None);
    }

    #[test]
    fn a_few_quick_frames_while_the_queue_fills_do_not_trip_it() {
        let mut pacer = Pacer::default();
        for _ in 0..3 {
            pacer.observe(0.001);
        }
        pacer.observe(1.0 / 60.0);
        for _ in 0..3 {
            pacer.observe(0.001);
        }
        assert_eq!(pacer.next_frame(1.0), None);
    }

    #[test]
    fn a_run_of_unpaced_frames_waits_a_refresh_from_then_on() {
        let mut pacer = Pacer::default();
        pacer.set_refresh(Some(120_000));
        for _ in 0..RUN {
            pacer.observe(0.002);
        }
        let next = pacer.next_frame(1.0).expect("paced by a timer");
        assert!((next - (1.0 + 1.0 / 120.0)).abs() < 1e-9);
        pacer.observe(1.0 / 120.0);
        assert!(pacer.next_frame(2.0).is_some());
    }

    #[test]
    fn an_unknown_refresh_assumes_sixty_hertz() {
        let mut pacer = Pacer::default();
        pacer.set_refresh(Some(0));
        for _ in 0..RUN {
            pacer.observe(0.001);
        }
        let next = pacer.next_frame(0.0).expect("paced");
        assert!((next - 1.0 / 60.0).abs() < 1e-9);
    }
}
