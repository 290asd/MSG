//! The slideshow's strip under two fingers. Tuned on recorded gestures (the `gestures` tool, 18.11.2026):
//! the touchpad comes as wheel lines in fractions, the fingers move only a few lines, and when they lift
//! Windows sends its coasting as one or two big chunks a quarter of a second later.
//!
//! So the strip is placed by how far the whole gesture went, not by speed: up to `EDGE` lines the next
//! picture only peeks (friction), past it the strip snaps on to it, and the coasting adds more slides
//! the further it throws. The strip follows that place with a little lag, which is its weight.

use std::time::{Duration, Instant};

/// No event for this long: the fingers are off, and the strip settles on a whole slide.
const SETTLE: Duration = Duration::from_millis(120);
/// No event for this long ends the gesture; until then Windows' coasting still adds to it.
const END: Duration = Duration::from_millis(600);
/// Lines of friction before the strip lets go of the picture (recorded peeks went 4–8 lines, moves 9–45).
// ponytail: tuned on one touchpad; the gestures tool records another.
pub const EDGE: f32 = 8.3;
/// How much of the next picture shows at the edge.
const PEEK: f32 = 0.25;
/// Past the edge, `SPREAD` more lines add one slide, then four, nine… (recorded flicks went 70–126 lines: 2–6 slides).
const SPREAD: f32 = 55.0;
/// How fast the strip catches up with the fingers (s): the weight.
const FOLLOW: f32 = 0.06;
/// How fast it settles on a slide (s).
const SNAP: f32 = 0.12;
/// A touchpad that tells points (not Windows) is taken at this many points per line.
pub const POINTS_PER_LINE: f32 = 50.0;

#[derive(Default)]
pub struct Roll {
    /// How far the strip is from the current slide, in points; more is towards the next one.
    pub y: f32,
    /// The gesture so far, in lines (more is towards the next one), and the slides it has already moved on.
    lines: f32,
    taken: f32,
    last: Option<Instant>,
    settled: bool,
    /// Mouse notches still to slide.
    notches: f32,
    stepped: Option<Instant>,
}

/// Where a gesture of `lines` puts the strip, in slides.
fn place(lines: f32) -> f32 {
    let a = lines.abs();
    let slides = if a <= EDGE { PEEK * a / EDGE } else { 1.0 + ((a - EDGE) / SPREAD).powi(2) };
    slides.copysign(lines)
}

impl Roll {
    pub fn moving(&self) -> bool {
        self.y != 0.0 || self.notches != 0.0 || (self.last.is_some() && !self.settled)
    }

    /// Whether a touchpad gesture is going on (so a whole line is part of it, not a mouse notch).
    pub fn in_gesture(&self, now: Instant) -> bool {
        self.last.is_some_and(|t| now - t < END)
    }

    /// The fingers (or Windows' coasting) moved `lines` this frame.
    pub fn drag(&mut self, lines: f32, now: Instant) {
        if !self.in_gesture(now) {
            self.lines = 0.0;
            self.taken = 0.0;
        }
        self.lines += lines;
        self.last = Some(now);
        self.settled = false;
        self.notches = 0.0;
    }

    /// A mouse notch: one more slide to slide.
    pub fn nudge(&mut self, by: i64, room: (usize, usize)) {
        self.last = None;
        self.notches = (self.notches + by as f32).clamp(-(room.1 as f32), room.0 as f32);
    }

    /// Moves the strip on; returns whether it still moves. `room` is how many slides there are (ahead, behind).
    pub fn step(&mut self, now: Instant, page: f32, room: (usize, usize)) -> bool {
        let dt = self.stepped.map_or(0.0, |t| (now - t).as_secs_f32().min(0.1));
        self.stepped = Some(now);
        let (goal, tau) = match self.last {
            Some(last) => {
                self.settled = now - last > SETTLE;
                let at = place(self.lines) - self.taken;
                if self.settled {
                    (at.round().clamp(-(room.1 as f32), room.0 as f32), SNAP)
                } else {
                    // A little past the first or last slide, as a rubber band.
                    (at.clamp(-(room.1 as f32) - PEEK, room.0 as f32 + PEEK), FOLLOW)
                }
            }
            None => (self.notches, SNAP),
        };
        let goal = goal * page;
        self.y = goal + (self.y - goal) * (-dt / tau).exp();
        if (self.y - goal).abs() < 0.5 {
            self.y = goal;
        }
        if self.last.is_some_and(|t| now - t > END) && self.y == goal {
            self.last = None;
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
        self.y -= by as f32 * page;
        if self.last.is_some() {
            self.taken += by as f32;
        } else {
            self.notches -= by as f32;
        }
        by
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays recorded events (ms from the start, wheel lines as Windows sent them, down is negative)
    /// at 120 fps until the strip stops; returns the slides it moved on and the furthest it went.
    fn replay(events: &[(f32, f32)]) -> (i64, f32) {
        let (mut roll, start, room, page) = (Roll::default(), Instant::now(), (99, 99), 1000.0);
        let (mut passed, mut furthest, mut next) = (0, 0.0f32, 0);
        for frame in 0..1000 {
            let ms = frame as f32 * 8.0;
            let now = start + Duration::from_secs_f32(ms / 1000.0);
            let mut lines = 0.0;
            while next < events.len() && events[next].0 <= ms {
                lines -= events[next].1;
                next += 1;
            }
            if lines != 0.0 {
                roll.drag(lines, now);
            }
            let moving = roll.step(now, page, room);
            passed += roll.take_page(page, room);
            furthest = furthest.max(passed as f32 + roll.y / page);
            if !moving && next == events.len() && !roll.in_gesture(now) {
                return (passed, furthest);
            }
        }
        panic!("the strip never stopped");
    }

    // Recorded on the user's touchpad (gestures-20261118-204001.csv).
    const EDGE_3: &[(f32, f32)] = &[(0.0, -3.275), (22.0, -1.133), (30.0, -1.000), (38.0, -0.783), (46.0, -0.542), (55.0, -0.367), (63.0, -0.242), (72.0, -0.175), (80.0, -0.100), (88.0, -0.075), (97.0, -0.042), (114.0, -0.025), (122.0, -0.025), (163.0, -0.025), (188.0, -0.025), (196.0, -0.025), (238.0, -0.025), (501.0, -0.025)];
    const NEXT_1: &[(f32, f32)] = &[(0.0, -2.617), (25.0, -1.200), (32.0, -1.075), (40.0, -0.958), (48.0, -0.733), (57.0, -0.275), (290.0, -2.325)];
    const NEXT_4: &[(f32, f32)] = &[(0.0, -2.592), (30.0, -1.200), (38.0, -2.700), (55.0, -3.233), (63.0, -1.758), (72.0, -1.742), (80.0, -1.642), (354.0, -29.275), (671.0, -0.958)];
    const MANY_1: &[(f32, f32)] = &[(0.0, -3.867), (27.0, -2.008), (35.0, -2.133), (43.0, -2.350), (52.0, -2.500), (60.0, -2.575), (68.0, -2.792), (77.0, 3.308), (85.0, -2.250), (93.0, -2.792), (369.0, -81.392), (735.0, -24.742)];
    const MANY_5: &[(f32, f32)] = &[(0.0, -3.475), (16.0, -1.617), (25.0, -1.617), (33.0, -1.617), (41.0, -1.742), (50.0, -1.808), (58.0, -1.917), (66.0, -1.883), (75.0, -1.858), (83.0, -1.817), (91.0, -1.792), (100.0, -0.808), (375.0, -46.200), (717.0, -2.817)];

    #[test]
    fn edge_peeks_and_comes_back() {
        let (passed, furthest) = replay(EDGE_3);
        assert_eq!(passed, 0);
        assert!(furthest > 0.15, "the next picture should show a little, went {furthest}");
    }

    #[test]
    fn next_moves_one() {
        assert_eq!(replay(NEXT_1).0, 1);
        assert_eq!(replay(NEXT_4).0, 1);
    }

    #[test]
    fn flick_moves_several() {
        let (five, one) = (replay(MANY_5).0, replay(MANY_1).0);
        assert!((2..=3).contains(&five), "{five}");
        assert!((4..=7).contains(&one), "{one}");
    }

    /// Replays a whole recording from the gestures tool and prints where each try ended:
    /// `MSG_GESTURES=<csv> cargo test --bin MSG replay_recording -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn replay_recording() {
        let text = std::fs::read_to_string(std::env::var("MSG_GESTURES").expect("MSG_GESTURES=<csv>")).unwrap();
        let mut tries: Vec<(String, Vec<(f32, f32)>)> = Vec::new();
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            let (key, ms, dy) = (format!("{} {}", f[0], f[1]), f[3].parse::<f32>().unwrap(), f[6].parse::<f32>().unwrap());
            if tries.last().is_none_or(|(k, _)| *k != key) {
                tries.push((key, Vec::new()));
            }
            tries.last_mut().unwrap().1.push((ms, dy));
        }
        for (key, mut events) in tries {
            let t0 = events[0].0;
            events.iter_mut().for_each(|e| e.0 -= t0);
            let lines: f32 = events.iter().map(|e| -e.1).sum();
            let (passed, furthest) = replay(&events);
            println!("{key}: {lines:6.1} lines -> {passed} slides (furthest {furthest:.2})");
        }
    }

    #[test]
    fn notches_slide_one_each() {
        let (mut roll, mut now) = (Roll::default(), Instant::now());
        roll.nudge(1, (99, 99));
        roll.nudge(1, (99, 99));
        let mut passed = 0;
        while roll.step(now, 1000.0, (99, 99)) {
            passed += roll.take_page(1000.0, (99, 99));
            now += Duration::from_millis(8);
        }
        assert_eq!((passed, roll.y), (2, 0.0));
    }

    #[test]
    fn last_slide_holds() {
        let (mut roll, now) = (Roll::default(), Instant::now());
        roll.drag(40.0, now);
        for frame in 1..200 {
            roll.step(now + Duration::from_millis(frame * 8), 1000.0, (0, 5));
            assert_eq!(roll.take_page(1000.0, (0, 5)), 0);
        }
        assert_eq!(roll.y, 0.0);
    }
}
