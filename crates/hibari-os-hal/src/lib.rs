//! Hardware abstraction for Hibari OS.
//!
//! Backends (ESP-IDF today, a native `no_std` kernel later) and board crates implement these
//! traits; the shell and apps only see the traits. A [`Board`] lists its capabilities so the
//! launcher can offer only the apps that can run.
#![cfg_attr(not(test), no_std)]

use hibari_os_core::{CapabilitySet, TouchEvent};

/// A rectangle in screen pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: u16,
    /// Top edge.
    pub y: u16,
    /// Width.
    pub w: u16,
    /// Height.
    pub h: u16,
}

impl Rect {
    /// Number of pixels the rectangle covers.
    #[must_use]
    pub const fn area(self) -> usize {
        self.w as usize * self.h as usize
    }

    /// Whether the rectangle fits inside a `width` x `height` screen.
    #[must_use]
    pub const fn fits(self, width: u16, height: u16) -> bool {
        self.x as u32 + self.w as u32 <= width as u32
            && self.y as u32 + self.h as u32 <= height as u32
    }
}

/// Shape of the visible screen area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Rectangular screen.
    Rect,
    /// Round screen inside the pixel rectangle.
    Round,
}

/// A pixel display. Pixels are RGB565, row by row.
pub trait Display {
    /// Error of the driver.
    type Error;

    /// Screen size in pixels: `(width, height)`.
    fn size(&self) -> (u16, u16);

    /// Shape of the visible area.
    fn shape(&self) -> Shape {
        Shape::Rect
    }

    /// Writes `pixels` (`area.area()` values) into `area`.
    ///
    /// # Errors
    /// Returns the driver error when the transfer fails.
    fn flush(&mut self, area: Rect, pixels: &[u16]) -> Result<(), Self::Error>;
}

/// A touch panel.
pub trait Touch {
    /// Error of the driver.
    type Error;

    /// The next touch sample, or `None` when there is nothing new.
    ///
    /// # Errors
    /// Returns the driver error when the panel cannot be read.
    fn poll(&mut self) -> Result<Option<TouchEvent>, Self::Error>;
}

/// Display brightness.
pub trait Backlight {
    /// Sets the level, 0 (off) to 255 (full).
    fn set_level(&mut self, level: u8);
}

/// A monotonic clock.
pub trait Clock {
    /// Milliseconds since boot; wraps after about 49 days.
    fn now_ms(&self) -> u32;
}

/// Describes one board (device): its name and what it can do.
pub trait Board {
    /// Name, for example `"waveshare-s3-amoled-2.06"`.
    const NAME: &'static str;
    /// What the board offers.
    const CAPABILITIES: CapabilitySet;
}

#[cfg(test)]
mod tests {
    use super::*;
    use hibari_os_core::Capability;

    struct Watch;
    impl Board for Watch {
        const NAME: &'static str = "test-watch";
        const CAPABILITIES: CapabilitySet =
            CapabilitySet::of(&[Capability::Display, Capability::Touch]);
    }

    #[test]
    fn rect_area_and_fit() {
        let r = Rect {
            x: 10,
            y: 20,
            w: 100,
            h: 50,
        };
        assert_eq!(r.area(), 5000);
        assert!(r.fits(110, 70));
        assert!(!r.fits(109, 70));
        assert!(
            !Rect {
                x: u16::MAX,
                y: 0,
                w: u16::MAX,
                h: 1
            }
            .fits(u16::MAX, 1)
        );
    }

    #[test]
    fn board_declares_capabilities() {
        assert_eq!(Watch::NAME, "test-watch");
        assert!(Watch::CAPABILITIES.contains(Capability::Touch));
    }
}
