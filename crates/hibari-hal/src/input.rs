//! Touch input and a gesture recognizer (tap, long press, swipe), with no allocation.

/// A point in screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    /// Horizontal position, left to right.
    pub x: i16,
    /// Vertical position, top to bottom.
    pub y: i16,
}

/// Where a touch is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Finger touched the panel.
    Down,
    /// Finger moved while touching.
    Move,
    /// Finger lifted.
    Up,
}

/// One raw touch sample from the panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TouchEvent {
    /// Life-cycle phase.
    pub phase: Phase,
    /// Position of the finger.
    pub point: Point,
    /// Monotonic time of the sample in milliseconds.
    pub t_ms: u32,
}

/// Direction of a swipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Towards the top of the screen.
    Up,
    /// Towards the bottom.
    Down,
    /// Towards the left.
    Left,
    /// Towards the right.
    Right,
}

/// A recognized gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    /// A short touch that barely moved.
    Tap(Point),
    /// A touch held in place past the long-press time.
    LongPress(Point),
    /// A touch that travelled far enough along one axis.
    Swipe(Direction),
}

/// Thresholds of the recognizer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GestureConfig {
    /// Largest movement (pixels, per axis) that still counts as staying in place.
    pub slop: u16,
    /// Smallest movement (pixels) along the main axis that counts as a swipe.
    pub swipe_min: u16,
    /// Time in milliseconds after which a held touch is a long press.
    pub long_press_ms: u32,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            slop: 12,
            swipe_min: 40,
            long_press_ms: 500,
        }
    }
}

/// Turns raw touch events into gestures. Feed every event to [`feed`](Self::feed) and call
/// [`tick`](Self::tick) regularly so a held finger becomes a long press.
#[derive(Clone, Copy, Debug, Default)]
pub struct GestureRecognizer {
    config: GestureConfig,
    start: Option<(Point, u32)>,
    last: Point,
    long_fired: bool,
}

fn dist(a: i16, b: i16) -> u32 {
    (i32::from(a) - i32::from(b)).unsigned_abs()
}

impl GestureRecognizer {
    /// A recognizer with the given thresholds.
    #[must_use]
    pub fn new(config: GestureConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Whether a touch is in progress.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.start.is_some()
    }

    fn moved_far(&self, from: Point, to: Point) -> bool {
        dist(from.x, to.x).max(dist(from.y, to.y)) > u32::from(self.config.slop)
    }

    /// Handles one touch event; gives a gesture when the touch ends in one.
    pub fn feed(&mut self, ev: TouchEvent) -> Option<Gesture> {
        match ev.phase {
            Phase::Down => {
                self.start = Some((ev.point, ev.t_ms));
                self.last = ev.point;
                self.long_fired = false;
                None
            }
            Phase::Move => {
                if self.start.is_some() {
                    self.last = ev.point;
                }
                None
            }
            Phase::Up => {
                let (start, _) = self.start.take()?;
                let fired = core::mem::take(&mut self.long_fired);
                if fired {
                    return None;
                }
                let end = ev.point;
                let (dx, dy) = (dist(end.x, start.x), dist(end.y, start.y));
                if !self.moved_far(start, end) {
                    return Some(Gesture::Tap(end));
                }
                if dx.max(dy) < u32::from(self.config.swipe_min) {
                    return None;
                }
                let horizontal = dx >= dy;
                Some(Gesture::Swipe(
                    match (horizontal, end.x >= start.x, end.y >= start.y) {
                        (true, true, _) => Direction::Right,
                        (true, false, _) => Direction::Left,
                        (false, _, true) => Direction::Down,
                        (false, _, false) => Direction::Up,
                    },
                ))
            }
        }
    }

    /// Call with the current time; gives a long press once a held touch passes the limit.
    pub fn tick(&mut self, now_ms: u32) -> Option<Gesture> {
        let (start, t0) = self.start?;
        if self.long_fired
            || self.moved_far(start, self.last)
            || now_ms.saturating_sub(t0) < self.config.long_press_ms
        {
            return None;
        }
        self.long_fired = true;
        Some(Gesture::LongPress(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn p(x: i16, y: i16) -> Point {
        Point { x, y }
    }
    const fn ev(phase: Phase, x: i16, y: i16, t_ms: u32) -> TouchEvent {
        TouchEvent {
            phase,
            point: p(x, y),
            t_ms,
        }
    }
    fn run(events: &[TouchEvent]) -> Option<Gesture> {
        let mut g = GestureRecognizer::default();
        events.iter().filter_map(|e| g.feed(*e)).last()
    }

    #[test]
    fn tap_within_slop() {
        let g = run(&[ev(Phase::Down, 100, 100, 0), ev(Phase::Up, 104, 98, 80)]);
        assert_eq!(g, Some(Gesture::Tap(p(104, 98))));
    }

    #[test]
    fn swipes_follow_the_main_axis() {
        let swipe = |x, y| {
            run(&[
                ev(Phase::Down, 100, 100, 0),
                ev(Phase::Move, x, y, 40),
                ev(Phase::Up, x, y, 80),
            ])
        };
        assert_eq!(swipe(200, 110), Some(Gesture::Swipe(Direction::Right)));
        assert_eq!(swipe(0, 90), Some(Gesture::Swipe(Direction::Left)));
        assert_eq!(swipe(110, 200), Some(Gesture::Swipe(Direction::Down)));
        assert_eq!(swipe(90, 0), Some(Gesture::Swipe(Direction::Up)));
    }

    #[test]
    fn medium_drag_is_neither_tap_nor_swipe() {
        assert_eq!(
            run(&[ev(Phase::Down, 100, 100, 0), ev(Phase::Up, 125, 100, 80)]),
            None
        );
    }

    #[test]
    fn long_press_fires_once_and_swallows_the_release() {
        let mut g = GestureRecognizer::default();
        assert_eq!(g.feed(ev(Phase::Down, 50, 50, 0)), None);
        assert_eq!(g.tick(300), None);
        assert_eq!(g.tick(500), Some(Gesture::LongPress(p(50, 50))));
        assert_eq!(g.tick(900), None);
        assert_eq!(g.feed(ev(Phase::Up, 50, 50, 950)), None);
        assert!(!g.is_active());
    }

    #[test]
    fn moving_cancels_long_press() {
        let mut g = GestureRecognizer::default();
        g.feed(ev(Phase::Down, 50, 50, 0));
        g.feed(ev(Phase::Move, 90, 50, 100));
        assert_eq!(g.tick(600), None);
    }

    #[test]
    fn stray_up_and_extreme_coordinates_are_safe() {
        let mut g = GestureRecognizer::default();
        assert_eq!(g.feed(ev(Phase::Up, 1, 1, 0)), None);
        let g2 = run(&[
            ev(Phase::Down, i16::MIN, i16::MIN, 0),
            ev(Phase::Up, i16::MAX, i16::MAX, 1),
        ]);
        assert_eq!(g2, Some(Gesture::Swipe(Direction::Right)));
    }
}
