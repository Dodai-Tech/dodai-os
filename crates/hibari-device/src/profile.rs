//! What a device is: its name, its screen and what it can do. One description serves the firmware
//! (as constants made at build time), the host preview and the build scripts.

use crate::capability::CapabilitySet;
use crate::render::{Hardware, Render};

/// Shape of the visible screen area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Shape {
    /// Rectangular screen.
    #[default]
    Rect,
    /// Round screen inside the pixel rectangle.
    Round,
}

/// A screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Visible width in pixels.
    pub width: u16,
    /// Visible height in pixels.
    pub height: u16,
    /// Shape of the visible area.
    pub shape: Shape,
    /// Columns and rows before the first visible pixel in the controller's memory; `(0, 0)` for
    /// most panels.
    pub offset: (u16, u16),
}

/// What it takes to build and flash firmware for a board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Firmware {
    /// The chip, as `espflash` names it (`"esp32s3"`).
    pub chip: &'static str,
    /// The Rust target triple.
    pub target: &'static str,
    /// Size of the flash in megabytes.
    pub flash_mb: u32,
}

/// Why a profile is not valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// The name is empty.
    EmptyName,
    /// A screen has a zero side.
    EmptyDisplay,
    /// The profile lists the `display` capability but has no screen, or the other way round.
    DisplayMismatch,
    /// A touch panel needs a screen to touch.
    TouchWithoutDisplay,
    /// `[render]` names a backend but the device has no screen to draw on.
    RenderWithoutDisplay,
    /// A `[hardware]` table with no RAM at all.
    NoRam,
    /// The text is not a valid device file; the message says what and where (an unknown field or
    /// capability, a duplicate, a bad value, a syntax error).
    #[cfg(feature = "toml")]
    Parse(std::string::String),
}

impl core::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyName => f.write_str("the device has no name"),
            Self::EmptyDisplay => f.write_str("the screen has a zero width or height"),
            Self::DisplayMismatch => {
                f.write_str("the `display` capability and the [display] table must go together")
            }
            Self::TouchWithoutDisplay => f.write_str("a device with `touch` needs a [display]"),
            Self::RenderWithoutDisplay => f.write_str("[render] needs a [display] to draw on"),
            Self::NoRam => f.write_str("[hardware] ram_kb must be more than zero"),
            #[cfg(feature = "toml")]
            Self::Parse(e) => write!(f, "{e}"),
        }
    }
}

/// A device: what it is called, its screen and its capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Name, for example `"waveshare-s3-amoled-2.06"`.
    pub name: &'static str,
    /// The screen, if there is one.
    pub display: Option<DisplayInfo>,
    /// What the device offers.
    pub capabilities: CapabilitySet,
    /// How to build firmware for it, for a real board; `None` for a simulated device.
    pub firmware: Option<Firmware>,
    /// What the chip has (RAM, PSRAM, FPU, GPU), if the board file says; the renderer choice reads it.
    pub hardware: Option<Hardware>,
    /// What the board file says about rendering.
    pub render: Render,
}

impl DeviceProfile {
    /// Whether the device has a screen.
    #[must_use]
    pub const fn has_screen(&self) -> bool {
        self.display.is_some()
    }

    /// The screen size in pixels, `(0, 0)` for a device without one.
    #[must_use]
    pub const fn screen_size(&self) -> (u16, u16) {
        match self.display {
            Some(d) => (d.width, d.height),
            None => (0, 0),
        }
    }

    /// Checks that the pieces agree with each other.
    ///
    /// # Errors
    /// The first [`ProfileError`] that applies.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.is_empty() {
            return Err(ProfileError::EmptyName);
        }
        if let Some(d) = self.display
            && (d.width == 0 || d.height == 0)
        {
            return Err(ProfileError::EmptyDisplay);
        }
        use crate::Capability::{Display, Touch};
        let shows = self.capabilities.contains(Display);
        if shows != self.display.is_some() {
            return Err(ProfileError::DisplayMismatch);
        }
        if self.capabilities.contains(Touch) && !shows {
            return Err(ProfileError::TouchWithoutDisplay);
        }
        if self.render.backend.is_some() && !shows {
            return Err(ProfileError::RenderWithoutDisplay);
        }
        if self.hardware.is_some_and(|h| h.ram_kb == 0) {
            return Err(ProfileError::NoRam);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability::{Display, Touch, Wifi};
    use std::string::ToString;

    const SCREEN: DisplayInfo = DisplayInfo {
        width: 410,
        height: 502,
        shape: Shape::Rect,
        offset: (22, 0),
    };

    fn profile(display: Option<DisplayInfo>, caps: &[crate::Capability]) -> DeviceProfile {
        DeviceProfile {
            name: "test",
            display,
            capabilities: CapabilitySet::of(caps),
            firmware: None,
            hardware: None,
            render: Render::default(),
        }
    }

    #[test]
    fn a_consistent_profile_is_valid_and_reports_its_screen() {
        let p = profile(Some(SCREEN), &[Display, Touch, Wifi]);
        assert_eq!(p.validate(), Ok(()));
        assert!(p.has_screen());
        assert_eq!(p.screen_size(), (410, 502));
        let headless = profile(None, &[Wifi]);
        assert_eq!(headless.validate(), Ok(()));
        assert!(!headless.has_screen());
        assert_eq!(headless.screen_size(), (0, 0));
    }

    #[test]
    fn each_inconsistency_has_its_own_error() {
        let mut p = profile(Some(SCREEN), &[Display]);
        p.name = "";
        assert_eq!(p.validate(), Err(ProfileError::EmptyName));
        let zero = DisplayInfo { width: 0, ..SCREEN };
        assert_eq!(
            profile(Some(zero), &[Display]).validate(),
            Err(ProfileError::EmptyDisplay)
        );
        assert_eq!(
            profile(Some(SCREEN), &[Wifi]).validate(),
            Err(ProfileError::DisplayMismatch)
        );
        assert_eq!(
            profile(None, &[Display]).validate(),
            Err(ProfileError::DisplayMismatch)
        );
        assert_eq!(
            profile(None, &[Touch]).validate(),
            Err(ProfileError::TouchWithoutDisplay)
        );
        for e in [
            ProfileError::EmptyName,
            ProfileError::EmptyDisplay,
            ProfileError::DisplayMismatch,
            ProfileError::TouchWithoutDisplay,
        ] {
            assert!(!e.to_string().is_empty());
        }
    }
}
