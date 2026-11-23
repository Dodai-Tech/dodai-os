//! CO5300 AMOLED controller (QSPI), as on the Waveshare ESP32-S3-Touch-AMOLED-2.06.
//!
//! Command set and start-up sequence follow Waveshare's `Arduino_CO5300` driver.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::OutputPin;
use hibari_os_hal::{Display, Rect};

const SLPOUT: u8 = 0x11;
const INVOFF: u8 = 0x20;
const DISPON: u8 = 0x29;
const CASET: u8 = 0x2A;
const PASET: u8 = 0x2B;
const RAMWR: u8 = 0x2C;
const MADCTL: u8 = 0x36;
const PIXFMT: u8 = 0x3A;
const BRIGHTNESS: u8 = 0x51;
const WCTRLD1: u8 = 0x53;
const WCE: u8 = 0x58;
const HBM_BRIGHTNESS: u8 = 0x63;
const SPIMODECTL: u8 = 0xC4;
const VENDOR_ACCESS: u8 = 0xFE;

/// The start-up register writes after sleep-out, in order: `(register, value)`.
pub const INIT_SEQUENCE: [(u8, u8); 8] = [
    (VENDOR_ACCESS, 0x00),
    (SPIMODECTL, 0x80),
    (PIXFMT, 0x55),
    (WCTRLD1, 0x20),
    (HBM_BRIGHTNESS, 0xFF),
    // `DISPON` is a command without data and is sent between these two by `init`.
    (BRIGHTNESS, 0xD0),
    (WCE, 0x00),
    (MADCTL, 0x00),
];

/// How many entries of [`INIT_SEQUENCE`] come before the display-on command.
const BEFORE_DISPLAY_ON: usize = 5;

/// What the controller needs from the bus. The board crate implements it over its QSPI peripheral.
pub trait QspiBus {
    /// Error of the bus.
    type Error;

    /// Writes `data` to register `reg` (command phase on one line).
    ///
    /// # Errors
    /// Returns the bus error.
    fn write_reg(&mut self, reg: u8, data: &[u8]) -> Result<(), Self::Error>;

    /// Starts a RAM write and sends `pixels` on four lines, each as big-endian RGB565.
    ///
    /// # Errors
    /// Returns the bus error.
    fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), Self::Error>;
}

/// Why a display call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error<B> {
    /// The QSPI bus failed.
    Bus(B),
    /// The reset pin failed.
    Pin,
    /// The area does not fit the screen, or the pixel count does not match it.
    BadArea,
}

/// Geometry of one panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Panel {
    /// Visible width in pixels.
    pub width: u16,
    /// Visible height in pixels.
    pub height: u16,
    /// Columns before the first visible one in controller memory.
    pub col_offset: u16,
    /// Rows before the first visible one in controller memory.
    pub row_offset: u16,
}

impl Panel {
    /// The Waveshare ESP32-S3-Touch-AMOLED-2.06 panel.
    pub const WAVESHARE_206: Self = Self {
        width: 410,
        height: 502,
        col_offset: 22,
        row_offset: 0,
    };
}

/// The display driver.
#[derive(Debug)]
pub struct Co5300<B, RST, D> {
    bus: B,
    reset: RST,
    delay: D,
    panel: Panel,
}

impl<B: QspiBus, RST: OutputPin, D: DelayNs> Co5300<B, RST, D> {
    /// A driver; call [`init`](Self::init) before drawing.
    pub const fn new(bus: B, reset: RST, delay: D, panel: Panel) -> Self {
        Self {
            bus,
            reset,
            delay,
            panel,
        }
    }

    fn pin<T>(result: Result<T, RST::Error>) -> Result<T, Error<B::Error>> {
        result.map_err(|_| Error::Pin)
    }

    /// Resets the controller, wakes it and runs the start-up sequence.
    ///
    /// # Errors
    /// Returns a bus or pin error.
    pub fn init(&mut self) -> Result<(), Error<B::Error>> {
        Self::pin(self.reset.set_high())?;
        self.delay.delay_ms(10);
        Self::pin(self.reset.set_low())?;
        self.delay.delay_ms(200);
        Self::pin(self.reset.set_high())?;
        self.delay.delay_ms(200);

        self.bus.write_reg(SLPOUT, &[]).map_err(Error::Bus)?;
        self.delay.delay_ms(120);
        for (i, (reg, value)) in INIT_SEQUENCE.iter().enumerate() {
            if i == BEFORE_DISPLAY_ON {
                self.bus.write_reg(DISPON, &[]).map_err(Error::Bus)?;
            }
            self.bus.write_reg(*reg, &[*value]).map_err(Error::Bus)?;
        }
        self.delay.delay_ms(10);
        self.bus.write_reg(INVOFF, &[]).map_err(Error::Bus)
    }

    /// Sets the brightness, 0 (dark) to 255.
    ///
    /// # Errors
    /// Returns the bus error.
    pub fn set_brightness(&mut self, level: u8) -> Result<(), Error<B::Error>> {
        self.bus.write_reg(BRIGHTNESS, &[level]).map_err(Error::Bus)
    }

    fn set_window(&mut self, area: Rect) -> Result<(), Error<B::Error>> {
        let x0 = area.x + self.panel.col_offset;
        let y0 = area.y + self.panel.row_offset;
        let (x1, y1) = (x0 + area.w - 1, y0 + area.h - 1);
        self.bus
            .write_reg(
                CASET,
                &[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8],
            )
            .map_err(Error::Bus)?;
        self.bus
            .write_reg(
                PASET,
                &[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8],
            )
            .map_err(Error::Bus)
    }
}

impl<B: QspiBus, RST: OutputPin, D: DelayNs> Display for Co5300<B, RST, D> {
    type Error = Error<B::Error>;

    fn size(&self) -> (u16, u16) {
        (self.panel.width, self.panel.height)
    }

    fn flush(&mut self, area: Rect, pixels: &[u16]) -> Result<(), Self::Error> {
        let fits = area.w > 0
            && area.h > 0
            && area.fits(self.panel.width, self.panel.height)
            && pixels.len() == area.area();
        if !fits {
            return Err(Error::BadArea);
        }
        self.set_window(area)?;
        // Start a memory write at the window's corner. Without it the panel carries on from wherever
        // the last write stopped, so a partial update lands in the wrong place or tears.
        self.bus.write_reg(RAMWR, &[]).map_err(Error::Bus)?;
        self.bus.write_pixels(pixels).map_err(Error::Bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::convert::Infallible;

    #[derive(Debug, PartialEq, Eq)]
    enum Op {
        Reg(u8, Vec<u8>),
        Pixels(usize),
        Reset(bool),
        Delay(u32),
    }

    /// One shared log, so the order of bus writes, pin changes and delays is checked together.
    #[derive(Default)]
    struct Log(std::rc::Rc<std::cell::RefCell<Vec<Op>>>);
    impl Clone for Log {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }
    struct Bus(Log, bool);
    impl QspiBus for Bus {
        type Error = &'static str;
        fn write_reg(&mut self, reg: u8, data: &[u8]) -> Result<(), &'static str> {
            if self.1 {
                return Err("bus down");
            }
            self.0.0.borrow_mut().push(Op::Reg(reg, data.to_vec()));
            Ok(())
        }
        fn write_pixels(&mut self, pixels: &[u16]) -> Result<(), &'static str> {
            self.0.0.borrow_mut().push(Op::Pixels(pixels.len()));
            Ok(())
        }
    }
    struct Pin(Log);
    impl embedded_hal::digital::ErrorType for Pin {
        type Error = Infallible;
    }
    impl OutputPin for Pin {
        fn set_low(&mut self) -> Result<(), Infallible> {
            self.0.0.borrow_mut().push(Op::Reset(false));
            Ok(())
        }
        fn set_high(&mut self) -> Result<(), Infallible> {
            self.0.0.borrow_mut().push(Op::Reset(true));
            Ok(())
        }
    }
    struct Delay(Log);
    impl DelayNs for Delay {
        fn delay_ns(&mut self, ns: u32) {
            self.0.0.borrow_mut().push(Op::Delay(ns / 1_000_000));
        }
    }

    fn driver(bus_down: bool) -> (Co5300<Bus, Pin, Delay>, Log) {
        let log = Log::default();
        let d = Co5300::new(
            Bus(log.clone(), bus_down),
            Pin(log.clone()),
            Delay(log.clone()),
            Panel::WAVESHARE_206,
        );
        (d, log)
    }
    fn reg(r: u8, d: &[u8]) -> Op {
        Op::Reg(r, d.to_vec())
    }

    #[test]
    fn init_resets_wakes_then_runs_the_sequence_in_order() {
        let (mut d, log) = driver(false);
        d.init().unwrap();
        let expected = vec![
            Op::Reset(true),
            Op::Delay(10),
            Op::Reset(false),
            Op::Delay(200),
            Op::Reset(true),
            Op::Delay(200),
            reg(0x11, &[]),
            Op::Delay(120),
            reg(0xFE, &[0x00]),
            reg(0xC4, &[0x80]),
            reg(0x3A, &[0x55]),
            reg(0x53, &[0x20]),
            reg(0x63, &[0xFF]),
            reg(0x29, &[]),
            reg(0x51, &[0xD0]),
            reg(0x58, &[0x00]),
            reg(0x36, &[0x00]),
            Op::Delay(10),
            reg(0x20, &[]),
        ];
        assert_eq!(*log.0.borrow(), expected);
    }

    #[test]
    fn init_reports_a_bus_failure() {
        let (mut d, _) = driver(true);
        assert_eq!(d.init(), Err(Error::Bus("bus down")));
    }

    #[test]
    fn flush_sets_the_window_with_the_panel_offset_then_sends_pixels() {
        let (mut d, log) = driver(false);
        let area = Rect {
            x: 10,
            y: 300,
            w: 4,
            h: 2,
        };
        d.flush(area, &[0; 8]).unwrap();
        // x: 10+22=32..35, y: 300..301
        let expected = vec![
            reg(0x2A, &[0, 32, 0, 35]),
            reg(0x2B, &[1, 44, 1, 45]),
            // Regression: a memory write must be started after the window, or a partial update
            // continues from where the last write stopped and tears.
            reg(0x2C, &[]),
            Op::Pixels(8),
        ];
        assert_eq!(*log.0.borrow(), expected);
    }

    #[test]
    fn flush_rejects_bad_areas_without_touching_the_bus() {
        let (mut d, log) = driver(false);
        assert_eq!(
            d.flush(
                Rect {
                    x: 0,
                    y: 0,
                    w: 2,
                    h: 2
                },
                &[0; 3]
            ),
            Err(Error::BadArea)
        );
        assert_eq!(
            d.flush(
                Rect {
                    x: 409,
                    y: 0,
                    w: 2,
                    h: 1
                },
                &[0; 2]
            ),
            Err(Error::BadArea)
        );
        assert_eq!(
            d.flush(
                Rect {
                    x: 0,
                    y: 501,
                    w: 1,
                    h: 2
                },
                &[0; 2]
            ),
            Err(Error::BadArea)
        );
        assert_eq!(
            d.flush(
                Rect {
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 0
                },
                &[]
            ),
            Err(Error::BadArea)
        );
        assert!(log.0.borrow().is_empty());
    }

    #[test]
    fn full_screen_flush_fits_and_brightness_is_one_write() {
        let (mut d, log) = driver(false);
        assert_eq!(Display::size(&d), (410, 502));
        d.set_brightness(0x40).unwrap();
        assert_eq!(*log.0.borrow(), vec![reg(0x51, &[0x40])]);
        let full = Rect {
            x: 0,
            y: 0,
            w: 410,
            h: 502,
        };
        d.flush(full, &vec![0; full.area()]).unwrap();
    }
}
