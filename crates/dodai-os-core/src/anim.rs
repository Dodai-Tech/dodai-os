//! Time-driven animation with integer math only (no floats, no allocation).
//!
//! Progress is in permille: 0 is the start and 1000 the end. A [`Tween`] turns the clock into
//! progress and an easing curve shapes it. The shell asks for a value at the current time on
//! every frame, so nothing is stored per frame and a dropped frame only skips ahead.

/// Progress at the end of an animation.
pub const FULL: u16 = 1000;

/// Fast start, gentle stop (cubic ease-out). Input and output are permille; input is clamped.
#[must_use]
pub const fn ease_out_cubic(p: u16) -> u16 {
    let p = if p > FULL { FULL } else { p } as u32;
    let inv = (FULL as u32) - p;
    (FULL as u32 - inv * inv * inv / 1_000_000) as u16
}

/// The value between `from` and `to` at `p` permille.
#[must_use]
pub const fn lerp(from: i32, to: i32, p: u16) -> i32 {
    (from as i64 + (to as i64 - from as i64) * p as i64 / FULL as i64) as i32
}

/// A fixed-length animation that started at a point in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tween {
    start_ms: u32,
    duration_ms: u32,
}

impl Tween {
    /// An animation of `duration_ms` starting at `start_ms`.
    #[must_use]
    pub const fn new(start_ms: u32, duration_ms: u32) -> Self {
        Self {
            start_ms,
            duration_ms,
        }
    }

    fn elapsed(self, now_ms: u32) -> u32 {
        let e = now_ms.wrapping_sub(self.start_ms);
        // A clock that reads before the start (or wrapped the other way) counts as "not begun".
        if e > u32::MAX / 2 { 0 } else { e }
    }

    /// Linear progress at `now_ms`, 0 to 1000.
    #[must_use]
    pub fn progress(self, now_ms: u32) -> u16 {
        if self.duration_ms == 0 {
            return FULL;
        }
        let e = u64::from(self.elapsed(now_ms)) * u64::from(FULL) / u64::from(self.duration_ms);
        e.min(u64::from(FULL)) as u16
    }

    /// Eased progress at `now_ms`, 0 to 1000.
    #[must_use]
    pub fn eased(self, now_ms: u32) -> u16 {
        ease_out_cubic(self.progress(now_ms))
    }

    /// Whether the animation has finished at `now_ms`.
    #[must_use]
    pub fn is_done(self, now_ms: u32) -> bool {
        self.progress(now_ms) >= FULL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easing_hits_the_ends_and_never_goes_backwards() {
        assert_eq!(ease_out_cubic(0), 0);
        assert_eq!(ease_out_cubic(FULL), FULL);
        assert_eq!(ease_out_cubic(u16::MAX), FULL, "input is clamped");
        let mut last = 0;
        for p in 0..=FULL {
            let e = ease_out_cubic(p);
            assert!(e >= last && e <= FULL);
            last = e;
        }
        assert!(ease_out_cubic(500) > 800, "fast start");
    }

    #[test]
    fn lerp_interpolates_in_both_directions_and_extremes() {
        assert_eq!(lerp(0, 100, 0), 0);
        assert_eq!(lerp(0, 100, 500), 50);
        assert_eq!(lerp(0, 100, FULL), 100);
        assert_eq!(lerp(100, -100, 750), -50);
        assert_eq!(lerp(i32::MIN, i32::MAX, FULL), i32::MAX);
        assert_eq!(lerp(i32::MIN, i32::MAX, 0), i32::MIN);
    }

    #[test]
    fn tween_progresses_and_finishes() {
        let t = Tween::new(1000, 200);
        assert_eq!(t.progress(1000), 0);
        assert_eq!(t.progress(1100), 500);
        assert_eq!(t.progress(1200), FULL);
        assert_eq!(t.progress(9999), FULL);
        assert!(!t.is_done(1199));
        assert!(t.is_done(1200));
        assert_eq!(t.eased(1000), 0);
        assert!(t.eased(1100) > 500);
    }

    #[test]
    fn tween_survives_the_clock_edges() {
        let t = Tween::new(1000, 200);
        assert_eq!(t.progress(500), 0, "before the start");
        let near_wrap = Tween::new(u32::MAX - 50, 200);
        assert_eq!(
            near_wrap.progress(49),
            500,
            "across the u32 wrap: 100 ms elapsed"
        );
        assert_eq!(
            Tween::new(5, 0).progress(5),
            FULL,
            "zero duration is already done"
        );
        assert!(Tween::new(0, u32::MAX).progress(u32::MAX / 2) <= 500);
    }
}
