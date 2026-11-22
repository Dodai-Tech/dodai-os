//! Dodai OS hosted backend: implements the HAL on a normal desktop OS, so the shell and apps
//! run in development without a board. Device profiles stand in for real devices (size, shape and
//! the capabilities the device would have).

use std::time::Instant;

use dodai_os_core::{Capability, CapabilitySet};
use dodai_os_hal::{Clock, Display, Rect, Shape};

/// A device the host can pretend to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Name used on the command line.
    pub name: &'static str,
    /// Screen width in pixels.
    pub width: u16,
    /// Screen height in pixels.
    pub height: u16,
    /// Shape of the visible area.
    pub shape: Shape,
    /// What the device offers.
    pub capabilities: CapabilitySet,
}

/// All profiles, in the order they are listed.
pub const PROFILES: [DeviceProfile; 5] = [
    DeviceProfile {
        name: "s3-amoled-2.06",
        width: 410,
        height: 502,
        shape: Shape::Rect,
        capabilities: CapabilitySet::of(&[
            Capability::Display,
            Capability::Touch,
            Capability::Button,
            Capability::Wifi,
            Capability::Ble,
            Capability::Battery,
            Capability::Rtc,
            Capability::Imu,
            Capability::Storage,
        ]),
    },
    DeviceProfile {
        name: "watch-round",
        width: 466,
        height: 466,
        shape: Shape::Round,
        capabilities: CapabilitySet::of(&[
            Capability::Display,
            Capability::Touch,
            Capability::Button,
            Capability::Ble,
            Capability::Battery,
            Capability::Imu,
            Capability::Storage,
        ]),
    },
    DeviceProfile {
        name: "p4-7in",
        width: 1024,
        height: 600,
        shape: Shape::Rect,
        capabilities: CapabilitySet::of(&[
            Capability::Display,
            Capability::Touch,
            Capability::Wifi,
            Capability::Storage,
        ]),
    },
    DeviceProfile {
        name: "headless",
        width: 0,
        height: 0,
        shape: Shape::Rect,
        capabilities: CapabilitySet::of(&[Capability::Wifi, Capability::Led, Capability::Storage]),
    },
    DeviceProfile {
        name: "desktop",
        width: 1280,
        height: 800,
        shape: Shape::Rect,
        capabilities: CapabilitySet::of(&[
            Capability::Display,
            Capability::Touch,
            Capability::Wifi,
            Capability::Storage,
        ]),
    },
];

impl DeviceProfile {
    /// The profile called `name`.
    #[must_use]
    pub fn by_name(name: &str) -> Option<&'static Self> {
        PROFILES.iter().find(|p| p.name == name)
    }

    /// Whether the device has a screen.
    #[must_use]
    pub const fn has_screen(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// The monotonic host clock.
#[derive(Debug)]
pub struct HostClock(Instant);

impl HostClock {
    /// A clock that starts at zero now.
    #[must_use]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for HostClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for HostClock {
    fn now_ms(&self) -> u32 {
        // Wraps after about 49 days, like the device clocks.
        (self.0.elapsed().as_millis() & u128::from(u32::MAX)) as u32
    }
}

/// A flush that does not fit the screen or has the wrong amount of pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadFlush;

/// An in-memory display (RGB565); a window or a PNG writer reads `pixels`.
#[derive(Clone, Debug)]
pub struct HostDisplay {
    width: u16,
    height: u16,
    shape: Shape,
    pixels: Vec<u16>,
}

impl HostDisplay {
    /// A black display for `profile`.
    #[must_use]
    pub fn new(profile: &DeviceProfile) -> Self {
        Self {
            width: profile.width,
            height: profile.height,
            shape: profile.shape,
            pixels: vec![0; usize::from(profile.width) * usize::from(profile.height)],
        }
    }

    /// The whole framebuffer, row by row.
    #[must_use]
    pub fn pixels(&self) -> &[u16] {
        &self.pixels
    }
}

impl Display for HostDisplay {
    type Error = BadFlush;

    fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    fn shape(&self) -> Shape {
        self.shape
    }

    fn flush(&mut self, area: Rect, pixels: &[u16]) -> Result<(), BadFlush> {
        if !area.fits(self.width, self.height) || pixels.len() != area.area() {
            return Err(BadFlush);
        }
        let (stride, w) = (usize::from(self.width), usize::from(area.w));
        for (row, src) in pixels
            .chunks_exact(w.max(1))
            .enumerate()
            .take(usize::from(area.h))
        {
            let start = (usize::from(area.y) + row) * stride + usize::from(area.x);
            self.pixels[start..start + w].copy_from_slice(src);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_have_unique_names_and_sane_sizes() {
        for (i, p) in PROFILES.iter().enumerate() {
            assert!(PROFILES[i + 1..].iter().all(|q| q.name != p.name));
            assert_eq!(
                p.has_screen(),
                p.capabilities.contains(Capability::Display),
                "{}",
                p.name
            );
        }
        assert_eq!(
            DeviceProfile::by_name("p4-7in").map(|p| p.width),
            Some(1024)
        );
        assert_eq!(DeviceProfile::by_name("nope"), None);
    }

    #[test]
    fn clock_is_monotonic() {
        let c = HostClock::new();
        let a = c.now_ms();
        assert!(c.now_ms() >= a);
    }

    #[test]
    fn display_flush_copies_rows_and_rejects_bad_areas() {
        let mut d = HostDisplay::new(DeviceProfile::by_name("watch-round").unwrap());
        assert_eq!(d.size(), (466, 466));
        assert_eq!(d.shape(), Shape::Round);
        let area = Rect {
            x: 1,
            y: 2,
            w: 2,
            h: 2,
        };
        assert_eq!(d.flush(area, &[1, 2, 3, 4]), Ok(()));
        assert_eq!(d.pixels()[2 * 466 + 1], 1);
        assert_eq!(d.pixels()[2 * 466 + 2], 2);
        assert_eq!(d.pixels()[3 * 466 + 1], 3);
        assert_eq!(d.pixels()[3 * 466 + 2], 4);
        assert_eq!(d.flush(area, &[1, 2, 3]), Err(BadFlush));
        assert_eq!(
            d.flush(
                Rect {
                    x: 465,
                    y: 0,
                    w: 2,
                    h: 1
                },
                &[0, 0]
            ),
            Err(BadFlush)
        );
    }

    #[test]
    fn headless_display_is_empty() {
        let d = HostDisplay::new(DeviceProfile::by_name("headless").unwrap());
        assert!(d.pixels().is_empty());
    }
}
