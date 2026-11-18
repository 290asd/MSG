//! The slideshow's strip under two fingers: it follows the touchpad, coasts on with the speed it was let go at, and settles on a whole slide.

use std::time::{Duration, Instant};

/// No touchpad event for this long means the fingers are off (Windows tells no end, and its own coasting comes as events).
pub const GAP: Duration = Duration::from_millis(120);
/// How long a flick coasts: the strip ends about speed × TAU further on, like iOS's deceleration.
// ponytail: fixed feel; measure on the touchpad if a flick goes too far or too short.
const TAU: f32 = 0.33;
/// The quickest settle, for a slow let-go or a mouse notch.
const SNAP: f32 = 0.08;
/// Past the first or last slide the strip follows the fingers this much, and at most a quarter of a slide.
const RUBBER: f32 = 0.35;

#[derive(Default)]
pub struct Roll {
    /// How far the strip is from the current slide, in points; more is towards the next one.
    pub y: f32,
    /// Points per second, smoothed.
    v: f32,
    last: Option<Instant>,
    stepped: Option<Instant>,
    /// Let go: where it settles and how fast it gets there.
    target: Option<(f32, f32)>,
}

impl Roll {
    pub fn moving(&self) -> bool {
        self.y != 0.0 || self.target.is_some()
    }

    /// The fingers move the strip by `dy`. `room` is how many slides there are (ahead, behind).
    pub fn drag(&mut self, dy: f32, now: Instant, page: f32, room: (usize, usize)) {
        let fresh = self.last.is_none_or(|t| now - t > GAP);
        let dt = if fresh { 1.0 / 60.0 } else { (now - self.last.unwrap()).as_secs_f32().max(0.004) };
        self.v = if fresh { dy / dt } else { 0.6 * self.v + 0.4 * dy / dt };
        self.last = Some(now);
        // Caught while it was settling: it stays where it is, under the fingers.
        self.target = None;
        let past = (dy > 0.0 && room.0 == 0 && self.y >= 0.0) || (dy < 0.0 && room.1 == 0 && self.y <= 0.0);
        self.y = if past { (self.y + dy * RUBBER).clamp(-page / 4.0, page / 4.0) } else { self.y + dy };
    }

    /// A mouse notch: the strip slides `by` slides on from where it is heading (`take_page` moves the slide on the way).
    pub fn nudge(&mut self, by: i64, page: f32, room: (usize, usize)) {
        let from = self.target.map_or(0.0, |(to, _)| (to / page).round());
        let pages = (from + by as f32).clamp(-(room.1 as f32), room.0 as f32);
        self.last = None;
        self.target = Some((pages * page, SNAP * 2.0));
    }

    fn release(&mut self, page: f32, room: (usize, usize)) {
        let pages = ((self.y + self.v * TAU) / page).round().clamp(-(room.1 as f32), room.0 as f32);
        let to = pages * page;
        // The strip keeps the speed it was let go at and slows down evenly towards the slide.
        let tau = if self.v.abs() > 1.0 { ((self.y - to).abs() / self.v.abs()).clamp(SNAP, TAU) } else { SNAP };
        self.target = Some((to, tau));
    }

    /// Moves the strip on; returns whether it still moves.
    pub fn step(&mut self, now: Instant, page: f32, room: (usize, usize)) -> bool {
        let dt = self.stepped.map_or(0.0, |t| (now - t).as_secs_f32());
        self.stepped = Some(now);
        if self.target.is_none() && self.y != 0.0 && self.last.is_none_or(|t| now - t > GAP) {
            self.release(page, room);
        }
        if let Some((to, tau)) = self.target {
            let x = (self.y - to) * (-dt / tau).exp();
            self.y = to + x;
            self.v = -x / tau;
            if x.abs() < 0.5 {
                self.y = to;
                self.v = 0.0;
                self.target = None;
            }
        }
        if self.y.abs() < 0.5 && self.target.is_none() {
            self.y = 0.0;
        }
        self.moving()
    }

    /// Past half a slide the next (1) or previous (-1) one is the current one; the strip is then measured from it.
    pub fn take_page(&mut self, page: f32, room: (usize, usize)) -> i64 {
        let by = if self.y > page / 2.0 && room.0 > 0 {
            1
        } else if self.y < -page / 2.0 && room.1 > 0 {
            -1
        } else {
            return 0;
        };
        let shift = by as f32 * page;
        self.y -= shift;
        if let Some((to, _)) = &mut self.target {
            *to -= shift;
        }
        by
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the strip at 60 fps until it stops, counting the slides it passed.
    fn run(roll: &mut Roll, mut now: Instant, room: (usize, usize)) -> i64 {
        let mut passed = 0;
        for _ in 0..600 {
            now += Duration::from_millis(16);
            let moving = roll.step(now, 1000.0, room);
            passed += roll.take_page(1000.0, room);
            if !moving {
                return passed;
            }
        }
        panic!("the strip never stopped");
    }

    #[test]
    fn flick_passes_slides_and_lands_on_one() {
        let mut roll = Roll::default();
        let mut now = Instant::now();
        for _ in 0..5 {
            now += Duration::from_millis(16);
            roll.drag(60.0, now, 1000.0, (99, 99));
        }
        let passed = run(&mut roll, now, (99, 99));
        assert!(passed >= 1, "a flick of 3750 pt/s went {passed} slides");
        assert_eq!(roll.y, 0.0);
    }

    #[test]
    fn slow_drag_springs_back() {
        let mut roll = Roll::default();
        let now = Instant::now();
        roll.drag(300.0, now, 1000.0, (99, 99));
        roll.drag(1.0, now + Duration::from_millis(200), 1000.0, (99, 99));
        assert_eq!(run(&mut roll, now + Duration::from_millis(200), (99, 99)), 0);
        assert_eq!(roll.y, 0.0);
    }

    #[test]
    fn notches_slide_one_each() {
        let mut roll = Roll::default();
        roll.nudge(1, 1000.0, (99, 99));
        roll.nudge(1, 1000.0, (99, 99));
        assert_eq!(run(&mut roll, Instant::now(), (99, 99)), 2);
        assert_eq!(roll.y, 0.0);
    }

    #[test]
    fn last_slide_holds() {
        let mut roll = Roll::default();
        let mut now = Instant::now();
        for _ in 0..20 {
            now += Duration::from_millis(16);
            roll.drag(200.0, now, 1000.0, (0, 5));
        }
        assert!(roll.y <= 250.0);
        assert_eq!(run(&mut roll, now, (0, 5)), 0);
    }
}
