//! PCF85063A real-time clock (I2C).
//!
//! Seven registers from `0x04` hold the time as two BCD digits each. Bit 7 of the seconds is the
//! oscillator-stop flag: set when the chip lost power, so the time is not to be trusted until it is
//! set again. The register map is from the datasheet; it has not been run against the chip yet.

use embedded_hal::i2c::I2c;
use hibari_hal::time::{DateTime, Rtc};

/// The chip's I2C address.
pub const ADDRESS: u8 = 0x51;

const REG_CONTROL_1: u8 = 0x00;
const REG_SECONDS: u8 = 0x04;
/// Control 1 with the clock running (STOP clear) and the 24-hour format (12_24 clear).
const CONTROL_RUN_24H: u8 = 0x00;
const OSCILLATOR_STOPPED: u8 = 0x80;

/// A decimal number below 100 as two BCD digits.
#[must_use]
pub const fn to_bcd(v: u8) -> u8 {
    ((v / 10) << 4) | (v % 10)
}

/// Two BCD digits as a number; `None` if either digit is above 9.
#[must_use]
pub const fn from_bcd(b: u8) -> Option<u8> {
    let (hi, lo) = (b >> 4, b & 0x0F);
    if hi > 9 || lo > 9 {
        None
    } else {
        Some(hi * 10 + lo)
    }
}

/// Decodes the seven registers from `0x04`. `None` when the oscillator-stop flag is set or a
/// register does not hold a valid date and time.
#[must_use]
pub fn decode(raw: [u8; 7]) -> Option<DateTime> {
    if raw[0] & OSCILLATOR_STOPPED != 0 {
        return None;
    }
    let time = DateTime {
        second: from_bcd(raw[0] & 0x7F)?,
        minute: from_bcd(raw[1] & 0x7F)?,
        hour: from_bcd(raw[2] & 0x3F)?,
        day: from_bcd(raw[3] & 0x3F)?,
        // raw[4] is the weekday; it is derived from the date instead.
        month: from_bcd(raw[5] & 0x1F)?,
        year: 2000 + u16::from(from_bcd(raw[6])?),
    };
    time.is_valid().then_some(time)
}

/// The seven registers for `time`, with the weekday derived from the date and the stop flag clear.
#[must_use]
pub fn encode(time: &DateTime) -> [u8; 7] {
    [
        to_bcd(time.second),
        to_bcd(time.minute),
        to_bcd(time.hour),
        to_bcd(time.day),
        time.weekday(),
        to_bcd(time.month),
        to_bcd((time.year.saturating_sub(2000) % 100) as u8),
    ]
}

/// The clock driver.
#[derive(Debug)]
pub struct Pcf85063<I> {
    i2c: I,
}

/// Why a write was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error<E> {
    /// The bus failed.
    Bus(E),
    /// The date or time is not valid, so it was not written.
    Invalid,
}

impl<I: I2c> Pcf85063<I> {
    /// A driver; call [`init`](Self::init) first.
    pub const fn new(i2c: I) -> Self {
        Self { i2c }
    }

    /// Makes sure the clock runs in the 24-hour format. Leaves the time alone.
    ///
    /// # Errors
    /// The I2C error.
    pub fn init(&mut self) -> Result<(), I::Error> {
        self.i2c.write(ADDRESS, &[REG_CONTROL_1, CONTROL_RUN_24H])
    }
}

impl<I: I2c> Rtc for Pcf85063<I> {
    type Error = Error<I::Error>;

    fn now(&mut self) -> Result<Option<DateTime>, Self::Error> {
        let mut raw = [0u8; 7];
        self.i2c
            .write_read(ADDRESS, &[REG_SECONDS], &mut raw)
            .map_err(Error::Bus)?;
        Ok(decode(raw))
    }

    fn set(&mut self, time: &DateTime) -> Result<(), Self::Error> {
        if !time.is_valid() {
            return Err(Error::Invalid);
        }
        let r = encode(time);
        let frame = [REG_SECONDS, r[0], r[1], r[2], r[3], r[4], r[5], r[6]];
        self.i2c.write(ADDRESS, &frame).map_err(Error::Bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::i2c::{ErrorKind, ErrorType, Operation};

    /// A chip: a register file, with a pointer set by the first written byte.
    struct Chip {
        regs: [u8; 0x12],
        pointer: usize,
        fail: bool,
    }

    impl Chip {
        fn with(time_regs: [u8; 7]) -> Self {
            let mut regs = [0u8; 0x12];
            regs[4..11].copy_from_slice(&time_regs);
            Self {
                regs,
                pointer: 0,
                fail: false,
            }
        }
    }

    impl ErrorType for Chip {
        type Error = ErrorKind;
    }

    impl I2c for Chip {
        fn transaction(&mut self, address: u8, ops: &mut [Operation<'_>]) -> Result<(), ErrorKind> {
            assert_eq!(address, ADDRESS);
            if self.fail {
                return Err(ErrorKind::Other);
            }
            for op in ops {
                match op {
                    Operation::Write(w) => {
                        self.pointer = usize::from(w[0]);
                        for (k, b) in w[1..].iter().enumerate() {
                            self.regs[self.pointer + k] = *b;
                        }
                    }
                    Operation::Read(r) => {
                        for (k, b) in r.iter_mut().enumerate() {
                            *b = self.regs[self.pointer + k];
                        }
                    }
                }
            }
            Ok(())
        }
    }

    fn some_time() -> DateTime {
        DateTime {
            year: 2026,
            month: 10,
            day: 28,
            hour: 23,
            minute: 59,
            second: 58,
        }
    }

    #[test]
    fn bcd_round_trips_and_rejects_non_digits() {
        for v in 0..100u8 {
            assert_eq!(from_bcd(to_bcd(v)), Some(v));
        }
        assert_eq!(to_bcd(59), 0x59);
        assert_eq!(from_bcd(0x1A), None);
        assert_eq!(from_bcd(0xA1), None);
    }

    #[test]
    fn a_set_time_reads_back_the_same() {
        let mut rtc = Pcf85063::new(Chip::with([OSCILLATOR_STOPPED, 0, 0, 1, 0, 1, 0]));
        assert_eq!(rtc.now().unwrap(), None, "unset clock: stop flag");
        rtc.set(&some_time()).unwrap();
        assert_eq!(rtc.now().unwrap(), Some(some_time()));
    }

    #[test]
    fn setting_the_time_writes_the_weekday_and_clears_the_stop_flag() {
        let mut rtc = Pcf85063::new(Chip::with([OSCILLATOR_STOPPED, 0, 0, 1, 0, 1, 0]));
        rtc.set(&some_time()).unwrap();
        let regs = &rtc.i2c.regs[4..11];
        assert_eq!(
            regs,
            [0x58, 0x59, 0x23, 0x28, 3, 0x10, 0x26],
            "28 Oct 2026 is a Wednesday"
        );
        assert_eq!(regs[0] & OSCILLATOR_STOPPED, 0);
    }

    #[test]
    fn the_century_bit_and_unused_bits_are_ignored() {
        let raw = [0x58, 0xD9, 0xE3, 0xE8, 0x03, 0x90, 0x26];
        assert_eq!(decode(raw), Some(some_time()));
    }

    #[test]
    fn garbage_registers_read_as_not_set() {
        assert_eq!(decode([0x7A, 0, 0, 1, 0, 1, 0]), None, "bad BCD");
        assert_eq!(
            decode([0x00, 0x00, 0x24, 0x01, 0, 0x01, 0x26]),
            None,
            "hour 24"
        );
        assert_eq!(
            decode([0x00, 0x00, 0x00, 0x31, 0, 0x04, 0x26]),
            None,
            "31 April"
        );
        assert_eq!(
            decode([0x00, 0x00, 0x00, 0x00, 0, 0x01, 0x26]),
            None,
            "day 0"
        );
    }

    #[test]
    fn an_invalid_time_is_refused_and_nothing_is_written() {
        let mut rtc = Pcf85063::new(Chip::with([0x11; 7]));
        let mut bad = some_time();
        bad.month = 13;
        assert_eq!(rtc.set(&bad), Err(Error::Invalid));
        assert_eq!(&rtc.i2c.regs[4..11], [0x11; 7]);
    }

    #[test]
    fn init_selects_24_hour_running_mode_and_leaves_the_time() {
        let mut chip = Chip::with([0x30, 0x20, 0x10, 1, 0, 1, 0]);
        chip.regs[0] = 0b0010_0010;
        let mut rtc = Pcf85063::new(chip);
        rtc.init().unwrap();
        assert_eq!(rtc.i2c.regs[0], 0);
        assert_eq!(&rtc.i2c.regs[4..7], [0x30, 0x20, 0x10]);
    }

    #[test]
    fn bus_errors_are_reported() {
        let mut chip = Chip::with([0; 7]);
        chip.fail = true;
        let mut rtc = Pcf85063::new(chip);
        assert_eq!(rtc.now(), Err(Error::Bus(ErrorKind::Other)));
        assert_eq!(rtc.set(&some_time()), Err(Error::Bus(ErrorKind::Other)));
        assert!(rtc.init().is_err());
    }
}
