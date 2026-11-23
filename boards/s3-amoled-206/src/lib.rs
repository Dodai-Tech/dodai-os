//! The Waveshare ESP32-S3-Touch-AMOLED-2.06: what the board is, and the bus its display sits on.
//!
//! [`PROFILE`] is generated from `board.toml` by `build.rs`, so the device is described once. The
//! firmware (`main.rs`) and the demos build on this library.
//!
//! GPIO map, for reference (the typed pins are taken where the peripherals are, because esp-hal
//! wants one owned handle per pin; source: Waveshare's `pin_config.h`):
//!
//! * LCD: SDIO0..3 = 4, 5, 6, 7; SCLK = 11; CS = 12; RESET = 8; TE = 13
//! * I2C: SDA = 15, SCL = 14 (touch 0x38, PMIC 0x34, RTC 0x51, IMU 0x6B)
//! * Touch: INT = 38, RESET = 9
//! * Buttons: BOOT = 0, PWR = 10
#![no_std]

extern crate alloc;

pub mod qspi;

/// The Fusuma backends linked into this firmware: none yet. The launcher still draws with its own integer
/// code; it moves to Fusuma and `vello_cpu` when the toolkit has the widgets it needs.
pub const LINKED: [hibari_device::Backend; 0] = [];

/// The renderer the build asks for, from a cargo feature; `None` leaves it to the board file and the
/// automatic rule. If several features are on, the lightest wins.
pub const BUILD_BACKEND: Option<hibari_device::Backend> = if cfg!(feature = "render-vello-cpu") {
    Some(hibari_device::Backend::VelloCpu)
} else if cfg!(feature = "render-vello-gpu") {
    Some(hibari_device::Backend::VelloGpu)
} else {
    None
};

include!(concat!(env!("OUT_DIR"), "/profile.rs"));

/// The screen, from the `[display]` table of `board.toml`.
pub const DISPLAY: hibari_device::DisplayInfo = match PROFILE.display {
    Some(d) => d,
    None => panic!("board.toml has no [display] table"),
};

/// QSPI clock of the display in MHz.
pub const LCD_QSPI_MHZ: u32 = 80;
