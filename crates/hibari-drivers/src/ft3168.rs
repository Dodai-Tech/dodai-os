//! FT3168 capacitive touch controller (I2C), single finger.
//!
//! One burst read of the five registers from `0x02` gives the finger count and its position, so
//! a sample costs a single I2C transaction.

use embedded_hal::i2c::I2c;
use hibari_hal::input::{Phase, Point, TouchEvent};
use hibari_hal::{Clock, Touch};

/// The controller's I2C address.
pub const ADDRESS: u8 = 0x38;

const REG_FINGER_NUM: u8 = 0x02;
const REG_POWER_MODE: u8 = 0xA5;
const POWER_MONITOR: u8 = 0x01;

/// A raw sample: how many fingers and where the first one is (panel pixels).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Number of fingers on the panel.
    pub fingers: u8,
    /// Horizontal position, 12 bits.
    pub x: u16,
    /// Vertical position, 12 bits.
    pub y: u16,
}

/// Decodes the five bytes read from register `0x02`: finger count, then X and Y as high and low
/// bytes. The upper four bits of each high byte are event flags, not position.
#[must_use]
pub const fn decode(raw: [u8; 5]) -> Sample {
    Sample {
        fingers: raw[0],
        x: ((raw[1] as u16 & 0x0F) << 8) | raw[2] as u16,
        y: ((raw[3] as u16 & 0x0F) << 8) | raw[4] as u16,
    }
}

/// The touch driver; turns samples into press, move and release events.
#[derive(Debug)]
pub struct Ft3168<I, C> {
    i2c: I,
    clock: C,
    last: Option<Point>,
}

impl<I: I2c, C: Clock> Ft3168<I, C> {
    /// A driver; call [`init`](Self::init) first.
    pub const fn new(i2c: I, clock: C) -> Self {
        Self {
            i2c,
            clock,
            last: None,
        }
    }

    /// Puts the controller in monitor power mode (wakes on touch).
    ///
    /// # Errors
    /// Returns the I2C error.
    pub fn init(&mut self) -> Result<(), I::Error> {
        self.i2c.write(ADDRESS, &[REG_POWER_MODE, POWER_MONITOR])
    }
}

impl<I: I2c, C: Clock> Touch for Ft3168<I, C> {
    type Error = I::Error;

    fn poll(&mut self) -> Result<Option<TouchEvent>, I::Error> {
        let mut raw = [0u8; 5];
        self.i2c.write_read(ADDRESS, &[REG_FINGER_NUM], &mut raw)?;
        let sample = decode(raw);
        let t_ms = self.clock.now_ms();
        let event = |phase, point| Some(TouchEvent { phase, point, t_ms });

        if sample.fingers == 0 {
            return Ok(self.last.take().and_then(|p| event(Phase::Up, p)));
        }
        let point = Point {
            x: i16::try_from(sample.x).unwrap_or(i16::MAX),
            y: i16::try_from(sample.y).unwrap_or(i16::MAX),
        };
        let previous = self.last.replace(point);
        Ok(match previous {
            None => event(Phase::Down, point),
            Some(p) if p != point => event(Phase::Move, point),
            Some(_) => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};
    use std::cell::Cell;

    /// Replays queued register blocks, and records writes.
    struct Fake {
        samples: std::collections::VecDeque<[u8; 5]>,
        writes: Vec<Vec<u8>>,
        fail: bool,
    }
    impl Fake {
        fn new(samples: &[[u8; 5]]) -> Self {
            Self {
                samples: samples.iter().copied().collect(),
                writes: vec![],
                fail: false,
            }
        }
    }
    impl ErrorType for Fake {
        type Error = ErrorKind;
    }
    impl I2c for Fake {
        fn transaction(&mut self, address: u8, ops: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
            assert_eq!(address, ADDRESS);
            if self.fail {
                return Err(ErrorKind::Other);
            }
            for op in ops {
                match op {
                    Operation::Write(w) => self.writes.push(w.to_vec()),
                    Operation::Read(r) => {
                        r.copy_from_slice(&self.samples.pop_front().unwrap_or([0; 5]))
                    }
                }
            }
            Ok(())
        }
    }
    struct Ticking(Cell<u32>);
    impl Clock for Ticking {
        fn now_ms(&self) -> u32 {
            self.0.set(self.0.get() + 10);
            self.0.get()
        }
    }

    const NONE: [u8; 5] = [0, 0, 0, 0, 0];
    const fn at(x: u16, y: u16) -> [u8; 5] {
        [1, (x >> 8) as u8, x as u8, (y >> 8) as u8, y as u8]
    }
    fn touch(samples: &[[u8; 5]]) -> Ft3168<Fake, Ticking> {
        Ft3168::new(Fake::new(samples), Ticking(Cell::new(0)))
    }

    #[test]
    fn decode_masks_the_event_flags_and_joins_12_bits() {
        let s = decode([1, 0b1100_0001, 0x2C, 0b0100_0010, 0xF0]);
        assert_eq!(
            s,
            Sample {
                fingers: 1,
                x: 0x12C,
                y: 0x2F0
            }
        );
        assert_eq!(decode([0, 0x0F, 0xFF, 0x0F, 0xFF]).x, 4095);
    }

    #[test]
    fn init_sets_monitor_mode() {
        let mut t = touch(&[]);
        t.init().unwrap();
        assert_eq!(t.i2c.writes, vec![vec![0xA5, 0x01]]);
    }

    #[test]
    fn a_touch_gives_down_move_up_and_skips_repeats() {
        let mut t = touch(&[NONE, at(100, 200), at(100, 200), at(120, 210), NONE, NONE]);
        let phases: Vec<_> = (0..6)
            .map(|_| t.poll().unwrap().map(|e| (e.phase, e.point.x, e.point.y)))
            .collect();
        assert_eq!(
            phases,
            [
                None,
                Some((Phase::Down, 100, 200)),
                None,
                Some((Phase::Move, 120, 210)),
                Some((Phase::Up, 120, 210)),
                None
            ]
        );
    }

    #[test]
    fn events_carry_increasing_timestamps() {
        let mut t = touch(&[at(1, 1), NONE]);
        let (a, b) = (
            t.poll().unwrap().unwrap().t_ms,
            t.poll().unwrap().unwrap().t_ms,
        );
        assert!(b > a);
    }

    #[test]
    fn bus_errors_are_returned_and_keep_the_state() {
        let mut t = touch(&[at(5, 5)]);
        t.poll().unwrap();
        t.i2c.fail = true;
        assert_eq!(t.poll().unwrap_err(), ErrorKind::Other);
        t.i2c.fail = false;
        assert_eq!(t.poll().unwrap().map(|e| e.phase), Some(Phase::Up));
    }
}
